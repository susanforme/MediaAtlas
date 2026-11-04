"""Single-EXE portable verification and the real std-only embedded exporter."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
RUSTC = os.environ.get("MEDIAATLAS_TEST_RUSTC") or shutil.which("rustc")


class PortableVerifierTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "compiled.exe"
        self.binary.write_bytes(b"binary-verifier-fixture")
        self.archive = self.root / "portable.zip"
        self.spec = {"arch": "x86_64"}

    def write_zip(self, content=None, extra=False):
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr("MediaAtlas.exe", content if content is not None else self.binary.read_bytes())
            if extra:
                archive.writestr("extra.txt", b"unrequested file")

    def test_zip_rejects_extra_members_and_changed_executable(self):
        self.write_zip(extra=True)
        with self.assertRaisesRegex(ValueError, "only MediaAtlas.exe"):
            release.inspect_portable_zip(self.archive, self.binary, self.spec)
        self.write_zip(content=b"different executable")
        with patch.object(release, "check_binary"), patch.object(release, "check_windows_crt"), patch.object(release, "run") as run:
            with self.assertRaisesRegex(ValueError, "binary mismatch"):
                release.inspect_portable_zip(self.archive, self.binary, self.spec)
            run.assert_not_called()

    def test_export_is_required_and_native_checks_are_retained(self):
        self.write_zip()
        with patch.object(release, "check_binary") as header, patch.object(release, "check_windows_crt") as crt, patch.object(release, "run", side_effect=subprocess.CalledProcessError(2, "export")):
            with self.assertRaises(subprocess.CalledProcessError):
                release.inspect_portable_zip(self.archive, self.binary, self.spec)
            header.assert_called_once()
            crt.assert_called_once()

    def test_complete_export_passes_and_missing_or_modified_notices_fail(self):
        self.write_zip()

        def export(_binary, command, target, **options):
            self.assertEqual(command, "--export-licenses")
            self.assertEqual(options["timeout"], 30)
            target.mkdir()
            for source in release.licenses():
                destination = target / source.relative_to(ROOT)
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, destination)
            if self.corrupt is True:
                (target / "LICENSE-STATUS.md").write_bytes(b"incorrect notice")
            elif self.corrupt is False:
                (target / "LICENSE-STATUS.md").unlink()

        with patch.object(release, "check_binary"), patch.object(release, "check_windows_crt"), patch.object(release, "run", side_effect=export):
            self.corrupt = None
            release.inspect_portable_zip(self.archive, self.binary, self.spec)
            for self.corrupt in (False, True):
                with self.subTest(corrupt=self.corrupt), self.assertRaisesRegex(ValueError, "license"):
                    release.inspect_portable_zip(self.archive, self.binary, self.spec)


class EmbeddedExporterTests(unittest.TestCase):
    @unittest.skipUnless(RUSTC, "rustc is needed for the actual embedded exporter")
    def test_compiled_export_works_without_source_files(self):
        with tempfile.TemporaryDirectory(prefix="mediaatlas-embedded-notices-") as temporary:
            directory = Path(temporary)
            source_root = directory / "sources"
            for source in release.licenses():
                target = source_root / source.relative_to(ROOT)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
            out = directory / "out"
            out.mkdir()
            suffix = ".exe" if os.name == "nt" else ""
            builder = directory / f"builder{suffix}"
            builder_source = directory / "builder.rs"
            module = json.dumps(str(ROOT / "apps/desktop/src-tauri/src/notices_build.rs"), ensure_ascii=False)
            builder_source.write_text(f'#[path = {module}] mod notices_build;\nfn main() {{ let args: Vec<_> = std::env::args_os().collect(); notices_build::generate(std::path::Path::new(&args[1]), std::path::Path::new(&args[2])).unwrap(); }}\n', encoding="utf-8")
            subprocess.run([RUSTC, "--edition=2021", str(builder_source), "-o", str(builder)], check=True, capture_output=True)
            # Exercise canonicalization on every host, even without Windows'
            # short-name TEMP path or macOS's symlinked temporary directory.
            builder_root = source_root / "docs" / ".."
            generated = subprocess.run([str(builder), str(builder_root), str(out)], check=True, capture_output=True, text=True)
            watched_paths = [
                Path(line.removeprefix("cargo:rerun-if-changed="))
                for line in generated.stdout.splitlines()
                if line.startswith("cargo:rerun-if-changed=")
            ]
            # Rust canonicalizes these paths: on Windows TEMP may use the
            # RUNNER~1 short name while Rust emits \\?\C:\Users\runneradmin.
            # Compare filesystem identity, including directory watches, not
            # path spelling. This also handles symlinked temp roots on macOS.
            notice_directory = builder_root / "docs/third-party-licenses"
            self.assertTrue(
                any(notice_directory.samefile(path) for path in watched_paths),
                "build output must watch the full license directory",
            )
            environment = {**os.environ, "OUT_DIR": str(out)}
            test_binary = directory / f"exporter-tests{suffix}"
            module_path = ROOT / "apps/desktop/src-tauri/src/notices.rs"
            subprocess.run([RUSTC, "--edition=2021", "--test", str(module_path), "-o", str(test_binary)], env=environment, check=True, capture_output=True)
            harness = directory / "exporter.rs"
            harness.write_text(f'#[path = {json.dumps(str(module_path), ensure_ascii=False)}] mod notices;\nfn main() {{ std::process::exit(match notices::handle_args(std::env::args_os().skip(1)) {{ Ok(true) => 0, Ok(false) => 3, Err(_) => 2 }}); }}\n', encoding="utf-8")
            executable = directory / f"exporter{suffix}"
            subprocess.run([RUSTC, "--edition=2021", str(harness), "-o", str(executable)], env=environment, check=True, capture_output=True)
            # Remove every embedded source before running either compiled binary.
            # Export must not depend on the developer checkout at runtime.
            shutil.rmtree(source_root)
            subprocess.run([str(test_binary)], check=True, capture_output=True)
            target = directory / "exported"
            subprocess.run([str(executable), "--export-licenses", str(target)], check=True, capture_output=True)
            release.check_licenses(target)
            expected = {source.relative_to(ROOT).as_posix() for source in release.licenses()}
            self.assertEqual({item.relative_to(target).as_posix() for item in target.rglob("*") if item.is_file()}, expected)
            for args, code in (([], 3), (["--unknown"], 2), (["--export-licenses", str(target)], 2)):
                self.assertEqual(subprocess.run([str(executable), *args], capture_output=True).returncode, code)


if __name__ == "__main__":
    unittest.main()
