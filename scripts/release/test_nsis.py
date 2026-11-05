"""Exercise the configured NSIS codec without bundling license materials.

Use MEDIAATLAS_TEST_MAKENSIS / MEDIAATLAS_TEST_7Z for non-PATH tools. The test
compiles and extracts a harmless file-only installer; it never executes it.
"""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
MAKENSIS = os.environ.get("MEDIAATLAS_TEST_MAKENSIS") or shutil.which("makensis")
SEVENZIP = os.environ.get("MEDIAATLAS_TEST_7Z") or shutil.which("7z") or shutil.which("7zz")


class NsisExtractionTests(unittest.TestCase):
    def test_selected_codec_and_license_inventory_agree(self):
        config = json.loads((ROOT / ".github/tauri.release.conf.json").read_text())
        # LZMA is Tauri's default NSIS codec and avoids the old zlib decoder path.
        self.assertEqual(config["bundle"]["windows"]["nsis"]["compression"], "lzma")
        native = json.loads((ROOT / "scripts/licenses/native-components.json").read_text())
        nsis = next(item for item in native if item["name"] == "NSIS installer stub and bundled standard plugins")
        self.assertIn("selects LZMA compression", nsis["scope"])
        notices = (ROOT / "docs/third-party-licenses/native/NSIS-COPYING.txt").read_text()
        self.assertIn("SPECIAL EXCEPTION FOR LZMA COMPRESSION MODULE", notices)

    @unittest.skipUnless(MAKENSIS and SEVENZIP, "requires native makensis and 7-Zip extraction tools")
    def test_actual_nsis_payload_roundtrips_without_license_materials(self):
        codec = json.loads((ROOT / ".github/tauri.release.conf.json").read_text())["bundle"]["windows"]["nsis"]["compression"]
        with tempfile.TemporaryDirectory(prefix="mediaatlas-nsis-probe-") as temporary:
            directory = Path(temporary)
            package = directory / "fixture.exe"
            payload = directory / "payload.bin"
            payload.write_bytes(b"MediaAtlas NSIS extraction regression\n" * 4096)

            def quoted(value):
                return str(value).replace("$", "$$").replace('"', '$\\"')

            lines = ["Unicode true", 'Name "MediaAtlas extractor fixture"',
                     f'OutFile "{quoted(package)}"', f"SetCompressor /SOLID {codec}",
                     "Section", 'SetOutPath "$INSTDIR"', f'File "{quoted(payload)}"']
            lines.append("SectionEnd")
            script = directory / "fixture.nsi"
            script.write_text("\n".join(lines) + "\n", encoding="utf-8")
            subprocess.run([MAKENSIS, "-V2", str(script)], check=True, capture_output=True, text=True)
            extracted = directory / "extracted"
            subprocess.run([SEVENZIP, "x", "-y", f"-o{extracted}", str(package)], check=True, capture_output=True, text=True)
            self.assertEqual((extracted / "payload.bin").read_bytes(), payload.read_bytes())
            release.check_package_contents(extracted)
            expected = {"payload.bin"}
            self.assertEqual({path.relative_to(extracted).as_posix() for path in extracted.rglob("*") if path.is_file()}, expected)


if __name__ == "__main__":
    unittest.main()
