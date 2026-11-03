import importlib.util
import json
from pathlib import Path
import struct
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tag = "v0.1.0-beta.1"
        self.version = self.tag[1:]
        self.commit = "1" * 40
        self.patch = patch.object(release, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        for filename in ("package.json", "apps/desktop/package.json", "apps/desktop/src-tauri/tauri.conf.json"):
            path = self.root / filename
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"version": self.version}))
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{self.version}"\n')
        (self.root / "Cargo.lock").write_text(f'[[package]]\nname = "mediaatlas-domain"\nversion = "{self.version}"\n')
        notice = self.root / "LICENSE-STATUS.md"
        notice.write_text("No license granted.\n")
        (self.root / "THIRD_PARTY_NOTICES.md").write_text("Third-party licenses.\n")
        (self.root / "docs").mkdir()
        (self.root / "docs/dependency-licenses.json").write_text("{}\n")
        license_dir = self.root / "docs/third-party-licenses/fixture"
        license_dir.mkdir(parents=True)
        (license_dir / "LICENSE").write_text("Fixture full license text.\n")
        self.input = self.root / "input"
        self.output = self.root / "output"

    def args(self):
        return type("Args", (), {"tag": self.tag, "commit": self.commit, "input": self.input, "output": self.output, "repository": "owner/repo"})()

    def fixture(self, include_optional=False):
        for spec in release.TARGETS:
            if not spec["required"] and not include_optional:
                continue
            directory = self.input / spec["target"]
            directory.mkdir(parents=True)
            assets = []
            for format_name in spec["formats"]:
                name = f"MediaAtlas_{self.version}_{spec['target']}.{format_name}"
                path = directory / name
                path.write_bytes(f"verified native package {name}".encode())
                assets.append({"name": name, "format": format_name, "bytes": path.stat().st_size, "sha256": release.sha256(path), "internal_architecture_verified": True})
            manifest = {"schema": 1, "tag": self.tag, "version": self.version, "commit": self.commit, **spec, "assets": assets, "tests_passed": True}
            (directory / "manifest.json").write_text(json.dumps(manifest))

    def modify_manifest(self, mutator):
        path = next(self.input.rglob("manifest.json"))
        manifest = json.loads(path.read_text())
        mutator(manifest)
        path.write_text(json.dumps(manifest))

    def test_versions_and_tag_must_agree(self):
        self.assertEqual(release.version(self.tag), self.version)
        for tag in ("v0.1.0", "v0.1.0-beta.0", "x0.1.0-beta.1", "v0.1.0-beta.1;bad"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.version(tag)
        (self.root / "package.json").write_text('{"version":"0.1.0"}')
        with self.assertRaisesRegex(ValueError, "mismatch"):
            release.version(self.tag)

    def test_lockfile_versions_checked(self):
        (self.root / "Cargo.lock").write_text('[[package]]\nname="mediaatlas-domain"\nversion="0.0.0"\n')
        with self.assertRaises(ValueError):
            release.version(self.tag)

    def test_native_headers_pe_elf_and_mach(self):
        path = self.root / "binary"
        for machine, expected in ((62, "x86_64"), (183, "aarch64")):
            header = bytearray(64)
            header[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", header, 18, machine)
            path.write_bytes(header)
            self.assertEqual(release.binary_arch(path), expected)
        for machine, expected in ((0x8664, "x86_64"), (0xAA64, "aarch64")):
            header = bytearray(64)
            header[:2] = b"MZ"
            struct.pack_into("<I", header, 60, 64)
            path.write_bytes(header + b"PE\0\0" + struct.pack("<H", machine))
            self.assertEqual(release.binary_arch(path), expected)
        for machine, expected in ((0x1000007, "x86_64"), (0x100000C, "aarch64")):
            path.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", machine) + bytes(56))
            self.assertEqual(release.binary_arch(path), expected)

    def test_pe_normal_and_delay_imports_reject_dynamic_visual_cpp_runtime(self):
        def fixture(normal, delayed):
            data = bytearray(8192)
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 60, 64)
            data[64:68] = b"PE\0\0"
            struct.pack_into("<H", data, 70, 1)
            struct.pack_into("<H", data, 84, 240)
            optional = 88
            struct.pack_into("<H", data, optional, 0x20B)
            struct.pack_into("<Q", data, optional + 24, 0x140000000)
            struct.pack_into("<I", data, optional + 60, 512)
            struct.pack_into("<I", data, optional + 108, 16)
            struct.pack_into("<IIII", data, optional + 240 + 8, 4096, 0x1000, 4096, 512)
            name_offset = 1536
            for index, libraries, descriptor_offset, size, field in ((1, normal, 512, 20, 12), (13, delayed, 768, 32, 4)):
                if libraries:
                    struct.pack_into("<I", data, optional + 112 + index * 8, descriptor_offset - 512 + 0x1000)
                for number, name in enumerate(libraries):
                    descriptor = descriptor_offset + number * size
                    if index == 13:
                        struct.pack_into("<I", data, descriptor, 1)
                    struct.pack_into("<I", data, descriptor + field, name_offset - 512 + 0x1000)
                    encoded = name.encode() + b"\0"
                    data[name_offset:name_offset + len(encoded)] = encoded
                    name_offset += len(encoded)
            path = self.root / "native.exe"
            path.write_bytes(data)
            return path
        path = fixture(["KERNEL32.dll", "USER32.dll"], ["ucrtbase.dll"])
        self.assertEqual(release.check_windows_crt(path), {"kernel32.dll", "user32.dll", "ucrtbase.dll"})
        for normal, delayed in ((["VCRUNTIME140_1.dll"], []), (["KERNEL32.dll"], ["MSVCP140.dll"])):
            with self.assertRaisesRegex(ValueError, r"Visual C\+\+ redistributable"):
                release.check_windows_crt(fixture(normal, delayed))

    def test_empty_truncated_and_wrong_arch_rejected(self):
        path = self.root / "binary"
        for content in (b"", b"MZ", b"not an executable"):
            path.write_bytes(content)
            with self.assertRaises((ValueError, struct.error)):
                release.check_binary(path, release.TARGETS[0])
        path.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", 0x100000C) + bytes(56))
        with self.assertRaisesRegex(ValueError, "architecture"):
            release.check_binary(path, release.TARGETS[0])

    def test_required_only_release_lists_unavailable_and_has_valid_checksums(self):
        self.fixture()
        release.aggregate(self.args())
        manifest = json.loads((self.output / "release-manifest.json").read_text())
        self.assertEqual(len(manifest["platforms"]), 4)
        self.assertEqual(len(manifest["unavailable_optional_targets"]), 2)
        self.assertIn("Not built", (self.output / "RELEASE_NOTES.md").read_text())
        for line in (self.output / "SHA256SUMS").read_text().splitlines():
            digest, filename = line.split("  ", 1)
            self.assertEqual(release.sha256(self.output / filename), digest)

    def test_full_matrix_release(self):
        self.fixture(include_optional=True)
        release.aggregate(self.args())
        manifest = json.loads((self.output / "release-manifest.json").read_text())
        self.assertEqual(len(manifest["platforms"]), 6)
        self.assertEqual(manifest["unavailable_optional_targets"], [])

    def test_missing_required_target_prevents_release(self):
        self.input.mkdir()
        with self.assertRaisesRegex(ValueError, "required target"):
            release.aggregate(self.args())

    def test_corruption_empty_and_missing_assets_prevent_release(self):
        self.fixture()
        asset = next(self.input.rglob("*.exe"))
        original = asset.read_bytes()
        for content in (b"", b"x" * len(original)):
            asset.write_bytes(content)
            with self.assertRaises(ValueError):
                release.aggregate(self.args())
            shutil.rmtree(self.output)
        asset.unlink()
        with self.assertRaises(ValueError):
            release.aggregate(self.args())

    def test_stale_commit_or_false_verification_rejected(self):
        self.fixture()
        self.modify_manifest(lambda data: data.update(commit="2" * 40))
        with self.assertRaisesRegex(ValueError, "provenance"):
            release.aggregate(self.args())

    def test_false_architecture_verification_rejected(self):
        self.fixture()
        self.modify_manifest(lambda data: data["assets"][0].update(internal_architecture_verified=False))
        with self.assertRaisesRegex(ValueError, "verification"):
            release.aggregate(self.args())

    def test_package_only_dryrun_cannot_claim_tests_or_publish(self):
        self.fixture()
        self.modify_manifest(lambda data: data.update(tests_passed=False))
        with self.assertRaisesRegex(ValueError, "tests not recorded"):
            release.aggregate(self.args())

    def test_incomplete_formats_rejected(self):
        self.fixture()
        self.modify_manifest(lambda data: data["assets"].pop())
        with self.assertRaisesRegex(ValueError, "formats"):
            release.aggregate(self.args())

    def test_extra_files_rejected(self):
        self.fixture()
        (next(self.input.iterdir()) / "unverified.exe").write_bytes(b"unknown")
        with self.assertRaisesRegex(ValueError, "unexpected"):
            release.aggregate(self.args())

    def test_target_metadata_rejected(self):
        self.fixture()
        self.modify_manifest(lambda data: data.update(arch="wrong"))
        with self.assertRaisesRegex(ValueError, "metadata"):
            release.aggregate(self.args())

    @unittest.skipUnless(shutil.which("dpkg-deb") and Path("/bin/true").is_file(), "native Debian tooling required")
    def test_real_debian_container_metadata_payload_and_license_validation(self):
        # An actual dpkg container with a native system binary exercises extraction;
        # this is a verifier test and never a MediaAtlas installation claim.
        package_root = self.root / "deb-root"
        control_dir = package_root / "DEBIAN"
        control_dir.mkdir(parents=True)
        control_dir.chmod(0o755)
        bin_dir = package_root / "usr/bin"
        bin_dir.mkdir(parents=True)
        shutil.copyfile("/bin/true", bin_dir / "mediaatlas-desktop")
        architecture = release.binary_arch(bin_dir / "mediaatlas-desktop")
        spec = next(item for item in release.TARGETS if item["os"] == "linux" and item["arch"] == architecture)
        deb_arch = {"x86_64": "amd64", "aarch64": "arm64"}[architecture]
        (control_dir / "control").write_text(f"Package: verifier-fixture\nVersion: {self.version}\nArchitecture: {deb_arch}\nMaintainer: Test <test@example.invalid>\nDescription: Release verifier extraction fixture\n")
        for source in release.licenses():
            destination = package_root / source.relative_to(self.root)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        package = self.root / "fixture.deb"
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(package_root), str(package)], check=True, capture_output=True)
        release.inspect_package(package, spec, self.version)
        with self.assertRaisesRegex(ValueError, "version mismatch"):
            release.inspect_package(package, spec, "0.0.0")
        wrong_spec = {**spec, "arch": "aarch64" if architecture == "x86_64" else "x86_64"}
        with self.assertRaisesRegex(ValueError, "architecture mismatch"):
            release.inspect_package(package, wrong_spec, self.version)
        shutil.copyfile("/bin/true", bin_dir / "WebKitWebProcess")
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(package_root), str(package)], check=True, capture_output=True)
        with self.assertRaisesRegex(ValueError, "unexpected bundled native payload"):
            release.inspect_package(package, spec, self.version)

    def test_bundled_license_material_must_match(self):
        directory = self.root / "extracted"
        directory.mkdir()
        for source in release.licenses():
            destination = directory / source.relative_to(self.root)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())
        release.check_licenses(directory)
        (directory / "LICENSE-STATUS.md").write_text("changed")
        with self.assertRaisesRegex(ValueError, "license material"):
            release.check_licenses(directory)

    def test_embedded_chromium_or_webkit_engine_cannot_be_bundled(self):
        directory = self.root / "extracted"
        directory.mkdir()
        for executable in ("msedgewebview2.exe", "chrome.exe", "icudtl.dat", "WebKit.framework"):
            path = directory / executable
            path.write_bytes(b"embedded engine is not permitted")
            with self.assertRaisesRegex(ValueError, "embedded browser engine"):
                release.check_licenses(directory)
            path.unlink()

    def test_optional_external_probe_executables_cannot_be_bundled(self):
        directory = self.root / "extracted"
        directory.mkdir()
        for executable in ("ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe"):
            path = directory / executable
            path.write_bytes(b"not allowed in distributed app")
            with self.assertRaisesRegex(ValueError, "media probe tools"):
                release.check_licenses(directory)
            path.unlink()

    def test_asset_path_traversal_rejected(self):
        self.fixture()
        self.modify_manifest(lambda data: data["assets"][0].update(name="../../unverified.exe"))
        with self.assertRaises(ValueError):
            release.aggregate(self.args())

    def test_linux_beta_only_distributes_deb_and_records_appimage_not_built(self):
        linux = next(item for item in release.build_matrix()["include"] if item["os"] == "linux")
        self.assertEqual(linux["bundles"], "deb")
        self.assertEqual(linux["formats"], ["deb"])
        self.assertEqual(linux["not_built_formats"][0]["format"], "AppImage")
        self.assertEqual(linux["not_built_formats"][0]["state"], "not_built")

    def test_appimage_cannot_be_enabled_without_native_license_gate(self):
        changed = [{**item, "formats": [*item["formats"], "AppImage"]} if item["os"] == "linux" else item for item in release.TARGETS]
        with patch.object(release, "TARGETS", changed), self.assertRaisesRegex(ValueError, "native source/license gate"):
            release.build_matrix()

    def test_no_optional_runner_is_scheduled_to_delay_required_release(self):
        matrix = release.build_matrix()
        self.assertEqual(len(matrix["include"]), 4)
        self.assertTrue(all(item["required"] for item in matrix["include"]))

    def test_matrix_has_four_required_and_two_optional_native_targets(self):
        self.assertEqual(sum(item["required"] for item in release.TARGETS), 4)
        self.assertEqual(len({item["target"] for item in release.TARGETS}), 6)
        self.assertTrue(all("latest" not in item["runner"] for item in release.TARGETS))


if __name__ == "__main__":
    unittest.main()
