"""Bundle sealing policy plus native macOS codesign and read-only DMG regressions."""
import importlib.util
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release_macos", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class SignaturePolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.app = Path(self.temporary.name) / "MediaAtlas Fixture.app"
        self.seal = self.app / "Contents/_CodeSignature/CodeResources"
        self.seal.parent.mkdir(parents=True)

    def test_release_overlay_requests_complete_ad_hoc_bundle_signing(self):
        config = json.loads((ROOT / ".github/tauri.release.conf.json").read_text())
        self.assertEqual(config["bundle"]["macOS"]["signingIdentity"], "-")

    def test_linker_only_signature_without_resource_seal_is_rejected(self):
        with patch.object(release, "run") as run:
            with self.assertRaisesRegex(ValueError, "resource seal"):
                release.verify_macos_signature(self.app)
            self.seal.write_bytes(b"")
            with self.assertRaisesRegex(ValueError, "resource seal"):
                release.verify_macos_signature(self.app)
            run.assert_not_called()

    def test_signature_display_success_cannot_bypass_failed_integrity_check(self):
        self.seal.write_text("Fixture resource seal")
        with patch.object(release, "run", side_effect=subprocess.CalledProcessError(1, "codesign")) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                release.verify_macos_signature(self.app)
            run.assert_called_once_with("codesign", "--verify", "--deep", "--strict", "--verbose=2", self.app, stderr=subprocess.STDOUT)

    def test_verified_ad_hoc_is_recorded_without_claiming_publisher_trust(self):
        self.seal.write_text("Fixture resource seal")
        with patch.object(release, "run", side_effect=["", "Signature=adhoc"]):
            result = release.verify_macos_signature(self.app)
        self.assertEqual(result, {"macos_code_signature": "ad-hoc", "macos_signature_verified": True})


@unittest.skipUnless(sys.platform == "darwin", "requires real macOS codesign, clang and hdiutil")
class NativeSignatureTests(unittest.TestCase):
    def setUp(self):
        for tool in ("codesign", "clang", "hdiutil"):
            self.assertIsNotNone(shutil.which(tool), f"macOS runner requires {tool}")
        self.temporary = tempfile.TemporaryDirectory(prefix="mediaatlas-signature-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.payload = self.root / "payload"
        self.app = self.payload / "MediaAtlas Fixture.app"
        contents = self.app / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        (contents / "Resources").mkdir()
        self.resource = contents / "Resources/fixture.txt"
        self.resource.write_text("Explicit synthetic signing fixture.\n")
        with (contents / "Info.plist").open("wb") as handle:
            plistlib.dump({"CFBundleIdentifier": "com.mediaatlas.signing-fixture", "CFBundleExecutable": "fixture", "CFBundleName": "MediaAtlas Fixture", "CFBundlePackageType": "APPL", "CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "1"}, handle)
        source = self.root / "fixture.c"
        source.write_text("int main(void) { return 0; }\n")
        self.binary = contents / "MacOS/fixture"
        release.run("clang", source, "-o", self.binary)
        release.run("codesign", "--force", "--sign", "-", "--timestamp=none", self.app, stderr=subprocess.STDOUT)

    def test_complete_signed_bundle_verifies_and_native_payload_runs(self):
        result = release.verify_macos_signature(self.app)
        self.assertTrue(result["macos_signature_verified"])
        self.assertEqual(result["macos_code_signature"], "ad-hoc")
        release.run(self.binary)

    def test_missing_resource_seal_reproduces_and_blocks_broken_bundle(self):
        (self.app / "Contents/_CodeSignature/CodeResources").unlink()
        with self.assertRaises(subprocess.CalledProcessError):
            release.run("codesign", "--verify", "--deep", "--strict", self.app, stderr=subprocess.STDOUT)
        with self.assertRaisesRegex(ValueError, "resource seal"):
            release.verify_macos_signature(self.app)

    def test_resource_changed_after_signing_is_rejected(self):
        self.resource.write_text("Changed after signing\n")
        with self.assertRaises(subprocess.CalledProcessError):
            release.verify_macos_signature(self.app)

    def test_readonly_mounted_dmg_preserves_bundle_seal(self):
        image = self.root / "fixture.dmg"
        release.run("hdiutil", "create", "-ov", "-format", "UDZO", "-fs", "HFS+", "-volname", "MediaAtlas Signing Fixture", "-srcfolder", self.payload, image)
        mount = self.root / "mount"
        release.run("hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, image)
        try:
            app = release.one(mount.glob("*.app"), "fixture app")
            self.assertTrue(release.verify_macos_signature(app)["macos_signature_verified"])
            self.assertEqual((app / "Contents/Resources/fixture.txt").read_bytes(), self.resource.read_bytes())
        finally:
            release.run("hdiutil", "detach", mount)


if __name__ == "__main__":
    unittest.main()
