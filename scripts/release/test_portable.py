"""Verify single-EXE portable packages without executing their payload."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class PortableVerifierTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "compiled.exe"
        self.binary.write_bytes(b"binary-verifier-fixture")
        self.archive = self.root / "portable.zip"
        self.spec = {"arch": "x86_64"}

    def write_zip(self, content=None, extra=None):
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr("MediaAtlas.exe", content if content is not None else self.binary.read_bytes())
            if extra:
                archive.writestr(extra, b"unrequested file")

    def test_no_desktop_config_bundles_license_resources(self):
        for filename in (".github/tauri.release.conf.json", "apps/desktop/src-tauri/tauri.conf.json"):
            with self.subTest(config=filename):
                config = json.loads((ROOT / filename).read_text())
                self.assertFalse(config["bundle"].get("resources"))

    def test_zip_rejects_extra_members_and_changed_executable(self):
        for extra in ("extra.txt", "LICENSE.md", "docs/third-party-licenses/LICENSE"):
            self.write_zip(extra=extra)
            with self.subTest(extra=extra), self.assertRaisesRegex(ValueError, "only MediaAtlas.exe"):
                release.inspect_portable_zip(self.archive, self.binary, self.spec)
        self.write_zip(content=b"different executable")
        with patch.object(release, "check_binary"), patch.object(release, "check_windows_crt"), patch.object(release, "run") as run:
            with self.assertRaisesRegex(ValueError, "binary mismatch"):
                release.inspect_portable_zip(self.archive, self.binary, self.spec)
            run.assert_not_called()

    def test_native_checks_are_retained_without_executing_the_application(self):
        self.write_zip()
        with patch.object(release, "check_binary") as header, patch.object(release, "check_windows_crt") as crt, patch.object(release, "run") as run:
            release.inspect_portable_zip(self.archive, self.binary, self.spec)
            header.assert_called_once()
            crt.assert_called_once()
            run.assert_not_called()

    def test_corrupt_zip_is_rejected_before_native_checks(self):
        self.write_zip()
        data = self.archive.read_bytes().replace(b"binary-verifier-fixture", b"Binary-verifier-fixture", 1)
        self.archive.write_bytes(data)
        with patch.object(release, "check_binary") as header, self.assertRaisesRegex(ValueError, "corrupt portable zip"):
            release.inspect_portable_zip(self.archive, self.binary, self.spec)
        header.assert_not_called()


if __name__ == "__main__":
    unittest.main()
