"""Cross-platform path and real-byte verification regressions for release notices."""
import contextlib
import importlib.util
import io
import json
import stat
from types import SimpleNamespace
from pathlib import Path, PurePosixPath, PureWindowsPath
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('mediaatlas_license_verify', Path(__file__).with_name('verify.py'))
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class MaterialPathTests(unittest.TestCase):
    def test_same_inventory_path_joins_posix_and_windows_roots(self):
        value = 'docs/third-party-licenses/native/rust/COPYRIGHT-library.html.txt'
        for root in [PurePosixPath('/checkout'), PureWindowsPath('C:/checkout')]:
            with self.subTest(root=root):
                actual = VERIFY.material_path(value, root)
                self.assertEqual(actual.parts[len(root.parts):], tuple(value.split('/')))
                self.assertEqual(actual, root / 'docs' / 'third-party-licenses' / 'native' / 'rust' / 'COPYRIGHT-library.html.txt')

    def test_rejects_absolute_traversal_aliases_and_encoded_paths_on_both_hosts(self):
        bad = [
            '/docs/third-party-licenses/test.txt',
            'C:/docs/third-party-licenses/test.txt',
            'C:docs/third-party-licenses/test.txt',
            '//server/share/docs/third-party-licenses/test.txt',
            r'docs\third-party-licenses\test.txt',
            'docs/third-party-licenses/../secret.txt',
            'docs/third-party-licenses/./test.txt',
            'docs//third-party-licenses/test.txt',
            'docs/third-party-licenses-evil/test.txt',
            'docs/third-party-licenses',
            'docs/third-party-licenses/',
            'docs/third-party-licenses/test.txt:stream',
            'docs/third-party-licenses/test.txt.',
            'docs/third-party-licenses/test.txt ',
            'docs/third-party-licenses/%2e%2e/secret.txt',
            'docs/third-party-licenses/%2Fsecret.txt',
            'docs/third-party-licenses/%5csecret.txt',
            'docs/third-party-licenses/test\0.txt',
        ]
        for root in [PurePosixPath('/checkout'), PureWindowsPath('C:/checkout')]:
            for value in bad:
                with self.subTest(root=root, value=value), self.assertRaisesRegex(ValueError, 'Unsafe notice path'):
                    VERIFY.material_path(value, root)


class IntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='mediaatlas-license-regression-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.notice = self.root / 'docs/third-party-licenses/texts/original.txt'
        self.notice.parent.mkdir(parents=True)
        self.notice_bytes = 'Copyright 原作者\r\nPermission: preserved.\r\n'.encode('utf-8')
        self.notice.write_bytes(self.notice_bytes)
        self.source = self.root / 'docs/third-party-licenses/sources/original.crate'
        self.source.parent.mkdir()
        self.source.write_bytes(b'\x1f\x8bOriginal source archive\x00\xff')
        (self.root / 'Cargo.lock').write_text('version = 4\npackage = []\n', encoding='utf-8')
        (self.root / 'pnpm-lock.yaml').write_text('lockfileVersion: 9\n', encoding='utf-8')
        for name in ['THIRD_PARTY_NOTICES.md', 'LICENSE.md']:
            (self.root / name).write_text('Fixture required notice.\n', encoding='utf-8')
        self.inventory = {
            'schema_version': 1,
            'missing_notices': [],
            'lockfiles': {name: VERIFY.digest(self.root / name) for name in ['Cargo.lock', 'pnpm-lock.yaml']},
            'dependencies': [{
                'ecosystem': 'native', 'name': 'fixture', 'version': '1.0', 'license': 'MIT',
                'notice_provenance': 'upstream_file',
                'notices': [{'path': self.notice.relative_to(self.root).as_posix(),
                             'sha256': VERIFY.digest(self.notice), 'origin': 'fixture upstream',
                             'original_path': 'LICENSE'}],
                'covered_source': self.source.relative_to(self.root).as_posix(),
                'source_sha256': VERIFY.digest(self.source),
            }],
        }
        self.inventory_path = self.root / 'docs/dependency-licenses.json'

    def check(self):
        self.inventory_path.write_text(json.dumps(self.inventory, ensure_ascii=False), encoding='utf-8')
        with patch.object(VERIFY, 'ROOT', self.root), patch.object(VERIFY.runpy, 'run_path', return_value={'npm_lock_packages': lambda: {}}), contextlib.redirect_stdout(io.StringIO()):
            VERIFY.verify()

    def test_original_non_ascii_crlf_and_binary_source_bytes_are_preserved(self):
        self.check()
        self.assertEqual(self.notice.read_bytes(), self.notice_bytes)

    def test_spdx_declaration_alone_cannot_count_as_complete_license_terms(self):
        self.inventory['dependencies'][0]['notices'][0]['original_path'] = 'LICENSE.spdx'
        with self.assertRaisesRegex(ValueError, 'SPDX declaration is not full license terms'):
            self.check()

    def test_changed_notice_bytes_fail(self):
        self.notice.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'Notice hash mismatch'):
            self.check()

    def test_changed_source_archive_fails(self):
        self.source.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'Covered source mismatch'):
            self.check()

    def test_changed_lockfile_fails(self):
        with (self.root / 'Cargo.lock').open('ab') as stream:
            stream.write(b'\n')
        with self.assertRaisesRegex(ValueError, 'Stale lockfile inventory'):
            self.check()

    def test_source_path_cannot_escape_material_directory(self):
        self.inventory['dependencies'][0]['covered_source'] = '../outside.crate'
        with self.assertRaisesRegex(ValueError, 'Unsafe notice path'):
            self.check()

    def test_notice_symlink_fails_even_when_bytes_match(self):
        outside = self.root / 'outside.txt'
        outside.write_bytes(self.notice_bytes)
        self.notice.unlink()
        try:
            self.notice.symlink_to(outside)
        except OSError as error:
            self.skipTest(f'Platform does not grant symlink creation: {error}')
        with self.assertRaisesRegex(ValueError, 'Unsafe material link'):
            self.check()

    def test_matching_notice_behind_external_ancestor_symlink_fails(self):
        external = tempfile.TemporaryDirectory(prefix='mediaatlas-license-outside-')
        self.addCleanup(external.cleanup)
        outside = Path(external.name)
        (outside / self.notice.name).write_bytes(self.notice_bytes)
        self.notice.unlink()
        self.notice.parent.rmdir()
        try:
            self.notice.parent.symlink_to(outside, target_is_directory=True)
        except OSError as error:
            self.skipTest(f'Platform does not grant symlink creation: {error}')
        with self.assertRaisesRegex(ValueError, 'Unsafe material link'):
            self.check()

    def test_matching_source_behind_ancestor_symlink_inside_root_also_fails(self):
        original = self.source.parent
        moved = self.root / 'source-alias-target'
        original.rename(moved)
        try:
            original.symlink_to(moved, target_is_directory=True)
        except OSError as error:
            self.skipTest(f'Platform does not grant symlink creation: {error}')
        with self.assertRaisesRegex(ValueError, 'Unsafe material link'):
            self.check()

    def test_missing_covered_source_fails_with_material_error(self):
        self.source.unlink()
        with self.assertRaisesRegex(ValueError, 'Missing or inaccessible material'):
            self.check()


class WindowsAliasLogicTests(unittest.TestCase):
    def test_reparse_attribute_is_rejected_on_any_host(self):
        path = SimpleNamespace(is_junction=lambda: False)
        info = SimpleNamespace(st_mode=stat.S_IFDIR, st_file_attributes=0x400)
        self.assertTrue(VERIFY.indirect_material(path, info))

    def test_windows_junction_probe_is_rejected_without_symlink_mode(self):
        path = SimpleNamespace(is_junction=lambda: True)
        info = SimpleNamespace(st_mode=stat.S_IFDIR, st_file_attributes=0)
        self.assertTrue(VERIFY.indirect_material(path, info))

    def test_ordinary_windows_directory_is_not_rejected(self):
        path = SimpleNamespace(is_junction=lambda: False)
        info = SimpleNamespace(st_mode=stat.S_IFDIR, st_file_attributes=0)
        self.assertFalse(VERIFY.indirect_material(path, info))


if __name__ == '__main__':
    unittest.main()
