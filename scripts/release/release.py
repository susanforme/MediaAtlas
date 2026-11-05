#!/usr/bin/env python3
"""Fail closed when release versions, native packages or downloaded assets disagree."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
TARGETS = json.loads((Path(__file__).with_name("targets.json")).read_text())


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args, **kwargs):
    try:
        return subprocess.check_output([str(arg) for arg in args], text=True, **kwargs).strip()
    except subprocess.CalledProcessError as error:
        # Tool versions and format diagnostics are normally on stdout. Keep
        # them in CI logs on failure instead of showing only a Python traceback.
        if error.output:
            print(error.output, file=sys.stderr)
        raise


def sha256(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def tag_version(tag):
    require(re.fullmatch(r"v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-beta\.[1-9][0-9]*)?", tag),
            "expected vX.Y.Z or vX.Y.Z-beta.N tag")
    return tag[1:]


def version(tag):
    expected = tag_version(tag)
    versions = {
        "package.json": json.loads((ROOT / "package.json").read_text())["version"],
        "apps/desktop/package.json": json.loads((ROOT / "apps/desktop/package.json").read_text())["version"],
        "tauri.conf.json": json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text())["version"],
        "Cargo.toml": tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"],
    }
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    for package in lock["package"]:
        if package["name"].startswith("mediaatlas-") and "source" not in package:
            versions[f"Cargo.lock:{package['name']}"] = package["version"]
    require(all(value == expected for value in versions.values()), f"tag/version mismatch: {versions}")
    return expected


def binary_arch(path):
    """Read native executable headers; a filename is never architecture evidence."""
    with path.open("rb") as handle:
        header = handle.read(64)
        if header[:4] == b"\x7fELF":
            require(header[4] == 2 and header[5] == 1, "expected 64-bit little-endian ELF")
            machine = struct.unpack_from("<H", header, 18)[0]
            return {62: "x86_64", 183: "aarch64"}.get(machine, "unknown")
        if header[:2] == b"MZ":
            require(len(header) == 64, "truncated PE header")
            handle.seek(struct.unpack_from("<I", header, 60)[0])
            pe = handle.read(6)
            require(pe[:4] == b"PE\0\0", "invalid PE signature")
            return {0x8664: "x86_64", 0xAA64: "aarch64"}.get(struct.unpack_from("<H", pe, 4)[0], "unknown")
        if header[:4] == b"\xcf\xfa\xed\xfe":
            cpu = struct.unpack_from("<I", header, 4)[0]
            return {0x1000007: "x86_64", 0x100000C: "aarch64"}.get(cpu, "unknown")
    raise ValueError(f"unsupported native executable: {path}")


def pe_imports(path):
    """Read normal and delay-load DLL imports from a native PE32+ application."""
    data = path.read_bytes()
    require(data[:2] == b"MZ" and len(data) >= 64, "invalid PE executable")
    pe = struct.unpack_from("<I", data, 60)[0]
    require(data[pe:pe + 4] == b"PE\0\0", "invalid PE signature")
    sections = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    require(optional_size >= 240 and struct.unpack_from("<H", data, optional)[0] == 0x20B, "expected complete PE32+ optional header")
    section_table = optional + optional_size
    require(section_table + sections * 40 <= len(data), "truncated PE sections")
    image_base = struct.unpack_from("<Q", data, optional + 24)[0]
    header_size = struct.unpack_from("<I", data, optional + 60)[0]
    directory_count = struct.unpack_from("<I", data, optional + 108)[0]

    def rva_offset(rva):
        if rva < header_size:
            require(rva < len(data), "PE header RVA out of range")
            return rva
        for index in range(sections):
            section = section_table + index * 40
            virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", data, section + 8)
            if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
                offset = raw_offset + rva - virtual_address
                require(offset < len(data), "PE section RVA out of range")
                return offset
        raise ValueError("unmapped PE import RVA")

    imported = set()
    for directory_index, descriptor_size, name_offset in ((1, 20, 12), (13, 32, 4)):
        if directory_count <= directory_index:
            continue
        rva = struct.unpack_from("<I", data, optional + 112 + directory_index * 8)[0]
        if not rva:
            continue
        offset = rva_offset(rva)
        for _ in range(4096):
            descriptor = data[offset:offset + descriptor_size]
            require(len(descriptor) == descriptor_size, "truncated PE imports")
            if not any(descriptor):
                break
            name_rva = struct.unpack_from("<I", descriptor, name_offset)[0]
            if directory_index == 13 and not (struct.unpack_from("<I", descriptor)[0] & 1):
                name_rva -= image_base
            start = rva_offset(name_rva)
            end = data.find(b"\0", start, start + 512)
            require(end >= start, "unterminated PE import name")
            imported.add(data[start:end].decode("ascii").lower())
            offset += descriptor_size
        else:
            raise ValueError("unterminated PE import descriptors")
    require(imported, "PE application has no inspectable DLL imports")
    return imported


def check_windows_crt(path):
    imports = pe_imports(path)
    require(not any(re.match(r"(?:vcruntime|msvcp|msvcr|concrt|vcomp|mfc)\d", name) for name in imports), "portable application dynamically requires a Visual C++ redistributable")
    return imports


def check_binary(path, spec):
    require(path.is_file() and path.stat().st_size > 0, f"missing/empty executable: {path}")
    require(binary_arch(path) == spec["arch"], f"wrong architecture in {path}")


def verify_macos_signature(app):
    """A linker signature alone does not seal a resource-bearing .app bundle."""
    seal = app / "Contents/_CodeSignature/CodeResources"
    require(seal.is_file() and seal.stat().st_size > 0, "missing macOS application resource seal; sign the complete bundle before creating the DMG")
    run("codesign", "--verify", "--deep", "--strict", "--verbose=2", app, stderr=subprocess.STDOUT)
    description = run("codesign", "--display", "--verbose=2", app, stderr=subprocess.STDOUT)
    return {
        "macos_code_signature": "ad-hoc" if "Signature=adhoc" in description else "present; publisher identity not verified",
        "macos_signature_verified": True,
    }


def one(paths, description):
    paths = list(paths)
    require(len(paths) == 1, f"expected exactly one {description}, found {paths}")
    return paths[0]


def check_licenses(directory):
    browser_engines = {"chrome", "chrome.exe", "chromium", "chromium.exe", "msedge.exe", "msedgewebview2.exe", "chrome.dll", "msedge.dll", "libcef.dll", "libcef.so", "icudtl.dat", "WebKit.framework", "Chromium Embedded Framework.framework", "Electron Framework.framework"}
    require(not any(path.name in browser_engines for path in directory.rglob("*")), "embedded browser engine must not be bundled; use the system WebView")
    forbidden = {"ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe"}
    require(not any(path.is_file() and path.name.lower() in forbidden for path in directory.rglob("*")), "external media probe tools must not be bundled")
    marker = one(
        (path for path in directory.rglob("LICENSE.md")
         if (path.parent / "THIRD_PARTY_NOTICES.md").is_file()
         and (path.parent / "docs/dependency-licenses.json").is_file()),
        "bundled project license",
    )
    resource_root = marker.parent
    for source in licenses():
        relative = source.relative_to(ROOT)
        installed = resource_root / relative
        require(installed.is_file() and sha256(installed) == sha256(source), f"bundled license material mismatch: {relative}")


def inspect_package(path, spec, expected_version):
    """Extract/mount each native package and inspect the shipped application's header."""
    inspection = {"payload_architecture": spec["arch"], "license_materials_verified": True, "embedded_browser_engine": False}
    with tempfile.TemporaryDirectory(prefix="mediaatlas-package-") as temporary:
        directory = Path(temporary)
        if path.suffix == ".exe":
            # The NSIS bootstrap itself may be x86 even when its payload is ARM64.
            run("7z", "x", "-y", f"-o{directory}", path)
            binary = one(directory.rglob("mediaatlas-desktop.exe"), "NSIS application payload")
            check_binary(binary, spec)
            check_windows_crt(binary)
            environment = {**os.environ, "MEDIAATLAS_VERIFY_BINARY": str(binary)}
            product_version = run("powershell", "-NoProfile", "-Command", "(Get-Item -LiteralPath $env:MEDIAATLAS_VERIFY_BINARY).VersionInfo.ProductVersion", env=environment)
            require(product_version in (expected_version, expected_version.split("-")[0]), "Windows payload product version mismatch")
            inspection["payload_version"] = product_version
            check_licenses(directory)
        elif path.suffix == ".deb":
            require(run("dpkg-deb", "-f", path, "Architecture") == {"x86_64": "amd64", "aarch64": "arm64"}[spec["arch"]], "deb architecture mismatch")
            require(run("dpkg-deb", "-f", path, "Version") == expected_version, "deb version mismatch")
            run("dpkg-deb", "-x", path, directory)
            binary = one((item for item in directory.rglob("mediaatlas-desktop") if item.is_file()), "deb executable")
            check_binary(binary, spec)
            # Debian relies on distro GTK/WebKit, not redistributed shared libraries.
            for item in directory.rglob("*"):
                if item.is_file() and item != binary:
                    require(not re.search(r"\.(?:so(?:\.\d+)*|dll|dylib)$", item.name, re.IGNORECASE), f"unexpected bundled shared library in deb: {item}")
                    with item.open("rb") as handle:
                        require(handle.read(4) != b"\x7fELF", f"unexpected bundled native payload in deb: {item}")
            check_licenses(directory)
        elif path.suffix == ".AppImage":
            check_binary(path, spec)
            path.chmod(path.stat().st_mode | 0o100)
            run(path, "--appimage-extract", cwd=directory)
            check_binary(one(directory.rglob("usr/bin/mediaatlas-desktop"), "AppImage executable"), spec)
            check_licenses(directory / "squashfs-root")
        elif path.suffix == ".dmg":
            mount = directory / "mount"
            run("hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, path)
            try:
                app = one(mount.glob("*.app"), "DMG app")
                with (app / "Contents/Info.plist").open("rb") as handle:
                    info = plistlib.load(handle)
                # Apple bundle short versions carry the numeric SemVer core.
                require(info["CFBundleShortVersionString"] in (expected_version, expected_version.split("-")[0]), "macOS bundle version mismatch")
                app_binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
                check_binary(app_binary, spec)
                require("/System/Library/Frameworks/WebKit.framework/" in run("otool", "-L", app_binary), "macOS application must link the system WebKit framework")
                inspection.update(verify_macos_signature(app))
                inspection["notarization"] = "not performed by this workflow"
                inspection["payload_version"] = info["CFBundleShortVersionString"]
                check_licenses(app / "Contents/Resources")
            finally:
                run("hdiutil", "detach", mount)
        else:
            raise ValueError(f"unexpected package format: {path}")
    return inspection


def licenses():
    paths = [ROOT / "LICENSE.md", ROOT / "THIRD_PARTY_NOTICES.md", ROOT / "docs/dependency-licenses.json"]
    full_texts = sorted(path for path in (ROOT / "docs/third-party-licenses").rglob("*") if path.is_file())
    require(full_texts, "release requires full third-party license texts")
    paths.extend(full_texts)
    require(all(path.is_file() and path.stat().st_size > 0 for path in paths), "release requires complete nonempty license materials")
    return paths


def inspect_portable_zip(path, executable, spec):
    """Verify the sole shipped EXE, including the notices it exports offline."""
    with zipfile.ZipFile(path) as archive:
        require(archive.namelist() == ["MediaAtlas.exe"], "portable ZIP must contain only MediaAtlas.exe")
        require(archive.testzip() is None, "corrupt portable zip")
        with tempfile.TemporaryDirectory(prefix="mediaatlas-portable-") as temporary:
            directory = Path(temporary)
            archive.extract("MediaAtlas.exe", directory)
            extracted = directory / "MediaAtlas.exe"
            check_binary(extracted, spec)
            check_windows_crt(extracted)
            require(sha256(extracted) == sha256(executable), "portable binary mismatch")
            # The native CI runner executes only the explicit export command;
            # it returns before initializing Tauri, the database or networking.
            notices = directory / "notices"
            run(extracted, "--export-licenses", notices, timeout=30)
            expected = {source.relative_to(ROOT).as_posix() for source in licenses()}
            actual = {item.relative_to(notices).as_posix() for item in notices.rglob("*") if item.is_file()}
            require(actual == expected, "embedded portable license file set mismatch")
            check_licenses(notices)


def stage(args):
    expected = version(args.tag)
    spec = next(item for item in TARGETS if item["target"] == args.target)
    require("AppImage" not in spec["formats"], "AppImage distribution requires the unfinished native source/license gate")
    profile = ROOT / "target" / args.target / "release"
    executable = profile / ("mediaatlas-desktop.exe" if spec["os"] == "windows" else "mediaatlas-desktop")
    check_binary(executable, spec)
    native_observations = {"architecture": binary_arch(executable)}
    if spec["os"] == "windows":
        native_observations["pe_imports"] = sorted(check_windows_crt(executable))
        native_observations["visual_cpp_runtime"] = "static; no redistributable DLL imports"
    if spec["os"] == "linux":
        needed = run("readelf", "-d", executable)
        for library in ("libwebkit2gtk-4.1.so.0", "libgtk-3.so.0", "libdbus-1.so.3"):
            require(f"[{library}]" in needed, f"Linux application must dynamically depend on system {library}")
        symbol_versions = run("readelf", "--version-info", executable)
        glibc_versions = [tuple(map(int, value.split("."))) for value in re.findall(r"GLIBC_(\d+\.\d+)", symbol_versions)]
        require(glibc_versions and max(glibc_versions) <= (2, 35) and "GLIBC_ABI_" not in symbol_versions, "Linux ELF exceeds the declared glibc 2.35 baseline")
        native_observations["elf_needed"] = re.findall(r"NEEDED.*\[(.*?)\]", needed)
        native_observations["glibc_max"] = ".".join(map(str, max(glibc_versions)))
    output = args.output / args.target
    require(not output.exists(), f"output already exists: {output}")
    output.mkdir(parents=True)
    assets = []
    patterns = {"exe": "nsis/*.exe", "dmg": "dmg/*.dmg", "deb": "deb/*.deb", "AppImage": "appimage/*.AppImage"}
    for format_name in spec["formats"]:
        filename = f"MediaAtlas_{expected}_{args.target}.{format_name}"
        destination = output / filename
        inspection = {"payload_architecture": spec["arch"], "license_materials_verified": True, "embedded_browser_engine": False}
        if format_name == "zip":
            with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.write(executable, "MediaAtlas.exe")
            inspect_portable_zip(destination, executable, spec)
            inspection["portable_single_executable"] = True
            inspection["license_materials"] = "embedded; explicit export verified byte for byte"
        else:
            package = one((profile / "bundle").glob(patterns[format_name]), format_name)
            require(package.stat().st_size > 0, f"empty package: {package}")
            inspection = inspect_package(package.resolve(), spec, expected)
            if format_name == "deb":
                depends = run("dpkg-deb", "-f", package, "Depends")
                for dependency in ("libwebkit2gtk-4.1-0", "libgtk-3-0", "libdbus-1-3", "xdg-utils"):
                    require(dependency in depends, f"Debian system runtime dependency missing: {dependency}")
            shutil.copyfile(package, destination)
        assets.append({"name": filename, "format": format_name, "bytes": destination.stat().st_size, "sha256": sha256(destination), "internal_architecture_verified": True, "package_verification": inspection})
    manifest = {"schema": 1, "version": expected, "tag": args.tag, "commit": args.commit, **spec, "assets": assets, "native_observations": native_observations, "tests_passed": args.tests_passed, "tests": "frontend tests and native Rust core tests passed" if args.tests_passed else "package validation only; test results not asserted", "signing": "no trusted publisher signature; macOS may use ad-hoc signing; no Apple notarization performed", "installation_tested": False}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Verified {args.target}: {', '.join(item['name'] for item in assets)}")


def aggregate(args):
    expected = version(args.tag)
    require(re.fullmatch(r"[0-9a-f]{40}", args.commit), "expected full git commit")
    args.output.mkdir(parents=True, exist_ok=True)
    require(not any(args.output.iterdir()), "aggregate output must be empty")
    manifests, unavailable, seen = [], [], set()
    for spec in TARGETS:
        directory = args.input / spec["target"]
        manifest_path = directory / "manifest.json"
        if not manifest_path.is_file():
            require(not spec["required"], f"missing required target: {spec['target']}")
            unavailable.append({"target": spec["target"], "state": "not_built", "reason": "outside the four required release targets; no optional runner job was scheduled"})
            continue
        manifest = json.loads(manifest_path.read_text())
        require(manifest["schema"] == 1 and manifest["tag"] == args.tag and manifest["version"] == expected and manifest["commit"] == args.commit, "manifest provenance mismatch")
        require(all(manifest.get(key) == value for key, value in spec.items()), "manifest target metadata mismatch")
        require(manifest.get("tests_passed") is True, "required native/frontend tests not recorded as passed")
        require(sorted(asset["format"] for asset in manifest["assets"]) == sorted(spec["formats"]), "incomplete formats")
        require({path.name for path in directory.iterdir()} == {"manifest.json", *(asset["name"] for asset in manifest["assets"])}, "unexpected/missing target files")
        for asset in manifest["assets"]:
            name = asset["name"]
            require(name == f"MediaAtlas_{expected}_{spec['target']}.{asset['format']}" and name not in seen, "unsafe/duplicate asset name")
            seen.add(name)
            package = directory / name
            require(package.is_file() and package.stat().st_size > 0 and package.stat().st_size == asset["bytes"], "asset missing/size mismatch")
            require(asset["internal_architecture_verified"] is True and sha256(package) == asset["sha256"], "asset checksum/verification mismatch")
            if spec["os"] == "macos":
                require(asset.get("package_verification", {}).get("macos_signature_verified") is True, "macOS package signature integrity was not verified")
            shutil.copyfile(package, args.output / name)
        manifests.append(manifest)
    license_paths = licenses()
    for license_path in license_paths[:3]:
        shutil.copyfile(license_path, args.output / license_path.name)
    with zipfile.ZipFile(args.output / "license-materials.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for license_path in license_paths:
            archive.write(license_path, license_path.relative_to(ROOT).as_posix())
    summary = {"schema": 1, "tag": args.tag, "version": expected, "commit": args.commit, "platforms": manifests, "unavailable_optional_targets": unavailable, "license_materials": [{"path": path.relative_to(ROOT).as_posix(), "bytes": path.stat().st_size, "sha256": sha256(path)} for path in license_paths]}
    (args.output / "release-manifest.json").write_text(json.dumps(summary, indent=2) + "\n")
    source = f"https://github.com/{args.repository}/tree/{args.commit}"
    lines = [f"# MediaAtlas {expected}", "", f"Source: {source}", f"Commit: `{args.commit}`", "", "Packages have no trusted publisher signature; macOS may use ad-hoc signing. Windows SmartScreen and macOS Gatekeeper may require explicit approval. Apple notarization and trusted publisher signing have not been performed. No automatic updater is enabled.", "", "Build validation checks frontend tests, native Rust core tests, package payload architecture, versions and SHA-256 integrity. Interactive installation, upgrade, uninstall, OS credential storage and live Provider access still require platform acceptance; successful compilation does not establish these results.", "", "Windows NSIS uses the official online WebView2 bootstrapper if the system runtime is missing; it does not bundle a fixed Chromium/WebView2 engine. Portable ZIP contains only MediaAtlas.exe, with complete license materials embedded at build time; use MediaAtlas.exe --export-licenses <new-directory> to export them explicitly. It requires an existing Microsoft WebView2 runtime; the application statically links the Visual C++ runtime, and its normal/delay PE imports are checked for unexpected redistributable DLL dependencies. macOS uses the system WKWebView/WebKit framework. Debian packages require system WebKitGTK 4.1, GTK3, libdbus-1 and xdg-utils; no browser engine or extra native shared libraries are bundled. Linux binaries are built on Ubuntu 22.04 (glibc 2.35). AppImage is not built in this release because native-library licensing and corresponding-source distribution checks are incomplete. ffmpeg/ffprobe are development fixture tools; the application does not invoke or bundle them.", "", "License and dependency notices: see LICENSE.md, THIRD_PARTY_NOTICES.md, dependency-licenses.json and license-materials.zip assets (the ZIP contains all full license texts with their repository paths); MediaAtlas is licensed under the MIT License; third-party components retain their own licenses. Provider services have their own access terms; no Provider credentials are shipped.", "", "| Target | Available packages |", "| --- | --- |"]
    for manifest in manifests:
        lines.append(f"| `{manifest['target']}` | {', '.join(asset['format'] for asset in manifest['assets'])} |")
    for item in unavailable:
        lines.append(f"| `{item['target']}` (optional) | Not built: {item['reason']} |")
    for manifest in manifests:
        for excluded in manifest.get("not_built_formats", []):
            lines.append(f"| `{manifest['target']}` / {excluded['format']} | Not built: {excluded['reason']} |")
    lines += ["", "Verify downloaded files using SHA256SUMS (`sha256sum -c SHA256SUMS` for a complete download, or compare an individual file's SHA-256). SHA-256 checksums detect changed bytes; they are not a signing identity.", ""]
    (args.output / "RELEASE_NOTES.md").write_text("\n".join(lines))
    checksum_lines = [f"{sha256(path)}  {path.name}" for path in sorted(args.output.iterdir()) if path.is_file()]
    (args.output / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n")
    print(f"Release ready: {len(manifests)} targets; {len(unavailable)} optional targets unavailable")


def build_matrix():
    # Schedule only the four required, mature combinations. Unavailable optional
    # runners can remain queued indefinitely and must not delay the release.
    enabled = [item for item in TARGETS if item["required"]]
    require(all("AppImage" not in item["formats"] for item in enabled), "AppImage distribution requires the unfinished native source/license gate")
    return {"include": enabled}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    preflight = subparsers.add_parser("preflight")
    preflight.add_argument("--tag", required=True)
    for command in ("stage", "aggregate"):
        command_parser = subparsers.add_parser(command)
        command_parser.add_argument("--tag", required=True)
        command_parser.add_argument("--commit", required=True)
        command_parser.add_argument("--output", type=Path, required=True)
        if command == "stage":
            command_parser.add_argument("--target", required=True, choices=[item["target"] for item in TARGETS])
            command_parser.add_argument("--tests-passed", action="store_true", help="record only after frontend and native Rust core tests passed")
        else:
            command_parser.add_argument("--input", type=Path, required=True)
            command_parser.add_argument("--repository", required=True)
    args = parser.parse_args()
    if args.command == "preflight":
        print(f"Validated release version: {version(args.tag)}")
        licenses()
        output = os.environ.get("GITHUB_OUTPUT")
        if output:
            with open(output, "a") as handle:
                handle.write("matrix=" + json.dumps(build_matrix(), separators=(",", ":")) + "\n")
    elif args.command == "stage":
        stage(args)
    else:
        aggregate(args)


if __name__ == "__main__":
    main()
