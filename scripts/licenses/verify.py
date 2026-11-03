#!/usr/bin/env python3
"""Fail closed if shipped notices are stale, missing, or changed."""
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import runpy
import stat
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def material_path(value, root):
    """Parse canonical repository POSIX paths before joining the host filesystem.

    Inventory paths are not native paths or URLs. Do not normalize traversal,
    decode escapes, or compare host-rendered path strings to POSIX prefixes.
    """
    if not isinstance(value, str) or not value or any(c in value for c in '\\%\0<>:"|?*'):
        raise ValueError('Unsafe notice path')
    relative = PurePosixPath(value)
    parts = value.split('/')
    if (relative.is_absolute() or PureWindowsPath(value).drive
            or tuple(parts[:2]) != ('docs', 'third-party-licenses') or len(parts) < 3
            or any(part in {'', '.', '..'} or part.endswith((' ', '.')) for part in parts)):
        raise ValueError('Unsafe notice path')
    return root.joinpath(*relative.parts)


def indirect_material(path, info):
    """Reject POSIX links and Windows junction/reparse aliases without following."""
    return (stat.S_ISLNK(info.st_mode)
            or bool(getattr(info, 'st_file_attributes', 0)
                    & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400))
            or bool(getattr(path, 'is_junction', lambda: False)()))


def material_file(value, root):
    """Require a real file under the actual checkout, with no aliased ancestor."""
    path = material_path(value, root)
    try:
        boundary = root.resolve(strict=True)
        current = root
        for part in ('', *PurePosixPath(value).parts):
            if part:
                current = current / part
            if indirect_material(current, current.lstat()):
                raise ValueError(f'Unsafe material link: {value}')
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(boundary) or not resolved.is_file():
            raise ValueError(f'Unsafe material boundary: {value}')
    except (OSError, RuntimeError) as error:
        raise ValueError(f'Missing or inaccessible material: {value}') from error
    return resolved


def verify():
    inventory = json.loads((ROOT / 'docs/dependency-licenses.json').read_text(encoding="utf-8"))
    if inventory.get('schema_version') != 1 or inventory.get('missing_notices'):
        raise ValueError('Incomplete license inventory')
    for name, expected in inventory['lockfiles'].items():
        if digest(ROOT / name) != expected:
            raise ValueError(f'Stale lockfile inventory: {name}; rerun generate.py')
    generated = runpy.run_path(str(ROOT / 'scripts/licenses/generate.py'))
    expected_packages = {('cargo', p['name'], p['version']) for p in tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding="utf-8"))['package'] if p.get('source')}
    expected_packages |= {('npm', n, v) for n, v in generated['npm_lock_packages']()}
    records = inventory['dependencies']
    actual_packages = {(e['ecosystem'], e['name'], e['version']) for e in records if e['ecosystem'] != 'native'}
    if actual_packages != expected_packages:
        raise ValueError('Locked dependency coverage mismatch')
    keys = [(e['ecosystem'], e['name'], e['version']) for e in records]
    if len(keys) != len(set(keys)):
        raise ValueError('Duplicate dependency inventory records')
    count = 0
    for entry in records:
        if not entry['notices'] or entry['license'] == 'UNSPECIFIED':
            raise ValueError(f'Missing license terms for {entry["name"]}')
        if entry['notice_provenance'] == 'upstream_metadata+standard_terms':
            if not entry.get('notice_limitation') or not entry.get('selected_license'):
                raise ValueError('Undisclosed upstream notice omission')
        for notice in entry['notices']:
            path = material_file(notice['path'], ROOT)
            if digest(path) != notice['sha256']:
                raise ValueError(f'Notice hash mismatch: {notice["path"]}')
            if not notice.get('origin') or not notice.get('original_path'):
                raise ValueError('Missing notice provenance')
            count += 1
        if entry.get('covered_source'):
            path = material_file(entry['covered_source'], ROOT)
            if digest(path) != entry['source_sha256']:
                raise ValueError(f'Covered source mismatch: {entry["name"]}')
    for required in ['THIRD_PARTY_NOTICES.md', 'LICENSE-STATUS.md']:
        if not (ROOT / required).is_file():
            raise ValueError(f'Missing {required}')
    print(f'License inventory verified: {len(records)} dependencies, {count} notice references, exact lockfile and source hashes.')


if __name__ == '__main__':
    try:
        verify()
    except (OSError, ValueError, KeyError) as error:
        print(f'License verification failed: {error}', file=sys.stderr)
        sys.exit(1)
