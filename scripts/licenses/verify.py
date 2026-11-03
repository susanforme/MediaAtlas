#!/usr/bin/env python3
"""Fail closed if shipped notices are stale, missing, or changed."""
import hashlib
import json
from pathlib import Path
import runpy
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
            relative = Path(notice['path'])
            if relative.is_absolute() or '..' in relative.parts or not str(relative).startswith('docs/third-party-licenses/'):
                raise ValueError('Unsafe notice path')
            path = ROOT / relative
            if path.is_symlink() or not path.is_file() or digest(path) != notice['sha256']:
                raise ValueError(f'Notice hash mismatch: {relative}')
            if not notice.get('origin') or not notice.get('original_path'):
                raise ValueError('Missing notice provenance')
            count += 1
        if entry.get('covered_source'):
            path = ROOT / entry['covered_source']
            if path.is_symlink() or digest(path) != entry['source_sha256']:
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
