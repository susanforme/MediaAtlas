#!/usr/bin/env python3
"""Inventory locked dependencies and preserve original notices. Python 3.11+, pnpm.

Run after pnpm install --frozen-lockfile and cargo fetch --locked. --fetch permits
HTTPS reads of upstream notices omitted from published package archives. Downloads
use normal proxy/CA settings. Generated files contain no local absolute paths.
"""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import subprocess
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import io
import tarfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'docs/third-party-licenses'
CACHE = OUT / 'provenance.json'
PATTERN = re.compile(r'^(licen[sc]e|copying|copyright|notice|patents|unlicense|authors)([.\-_]|$)', re.I)
FETCH = '--fetch' in sys.argv
PROVENANCE = json.loads(CACHE.read_text(encoding="utf-8")) if CACHE.exists() else {}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetch(url):
    if not FETCH:
        raise RuntimeError(f'Need --fetch for {url}')
    request = urllib.request.Request(url, headers={'User-Agent': 'MediaAtlas-license-inventory/1'})
    with urllib.request.urlopen(request, timeout=35) as response:
        data = response.read(12 * 1024 * 1024 + 1)
    if len(data) > 12 * 1024 * 1024:
        raise RuntimeError(f'Oversize license input: {url}')
    return data


def save_text(data, original, origin):
    # Preserve byte-for-byte original notice content, including authors/copyrights.
    digest = sha(data)
    target = OUT / 'texts' / (digest + '.txt')
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    return {'path': target.relative_to(ROOT).as_posix(), 'sha256': digest,
            'original_path': original, 'origin': origin}


def notice_files(directory):
    return sorted(p for p in directory.rglob('*') if p.is_file()
                  and PATTERN.match(p.name) and 'node_modules' not in p.relative_to(directory).parts)


def local_notices(directory, source):
    return [save_text(p.read_bytes(), p.relative_to(directory).as_posix(), source)
            for p in notice_files(directory)]


def upstream_notices(key, repository, revision, subdir=''):
    if key in PROVENANCE:
        return PROVENANCE[key]
    repository = (repository or '').removeprefix('git+').removesuffix('.git').replace('git://github.com/', 'https://github.com/')
    if repository.count('/') == 1 and ':' not in repository:
        repository = 'https://github.com/' + repository
    match = re.match(r'https?://github.com/([^/]+/[^/#]+)', repository)
    if not match:
        return []
    repo = match[1]
    candidates = ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE.txt', 'LICENSE.md',
                  'COPYING', 'COPYRIGHT', 'NOTICE', 'LICENSE-APACHE-2.0', 'LICENSE_APACHE-2.0', 'LICENSE_MIT', 'UNLICENSE',
                  'LICENSES/MIT.txt', 'LICENSES/Apache-2.0.txt', 'LICENSES/MPL-2.0.txt', 'LICENSES/BSL-1.0.txt']
    refs = [revision] if revision else ['master', 'main']
    # Packages in workspaces often omit root notices. Fetch their exact VCS commit.
    result = []
    for ref in refs:
        for name in candidates:
            url = f'https://raw.githubusercontent.com/{repo}/{ref}/{name}'
            try:
                data = fetch(url)
                if data and b'<html' not in data[:100].lower():
                    result.append(save_text(data, name, url))
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise
        if result:
            break
    if not result:
        for ref in refs:
            url = f'https://codeload.github.com/{repo}/tar.gz/{ref}'
            try:
                archive = tarfile.open(fileobj=io.BytesIO(fetch(url)), mode='r:gz')
                for member in archive.getmembers():
                    if member.isfile() and PATTERN.match(Path(member.name).name):
                        data = archive.extractfile(member).read()
                        result.append(save_text(data, member.name.split('/', 1)[-1], url))
                if result:
                    break
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise
    PROVENANCE[key] = result
    return result


def cargo_inventory():
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    bases = list((cargo_home / 'registry/src').glob('*'))
    entries = []
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding="utf-8"))['package']:
        if not package.get('source'):
            continue
        name, version = package['name'], package['version']
        directory = next((b / f'{name}-{version}' for b in bases if (b / f'{name}-{version}').is_dir()), None)
        if directory is None:
            raise RuntimeError(f'cargo fetch --locked required for {name}@{version}')
        meta = tomllib.loads((directory / 'Cargo.toml').read_text(encoding="utf-8"))['package']
        source = f'https://static.crates.io/crates/{name}/{name}-{version}.crate'
        notices = local_notices(directory, source)
        # libappindicator-sys omits repository but shares the parent workspace.
        repository = meta.get('repository') or ('https://github.com/tauri-apps/libappindicator-rs' if name == 'libappindicator-sys' else None)
        vcs = json.loads((directory / '.cargo_vcs_info.json').read_text(encoding="utf-8")) if (directory / '.cargo_vcs_info.json').exists() else {}
        if not notices and name == 'selectors':
            mpl = next(bases[0].glob('cssparser-*/LICENSE'))
            notices = [save_text(mpl.read_bytes(), 'LICENSE (standard MPL 2.0 text)', 'https://static.crates.io/crates/cssparser/' + mpl.parent.name + '.crate'),
                       save_text((directory / 'lib.rs').read_bytes().split(b'*/', 1)[0] + b'*/\n', 'lib.rs MPL notice', source)]
        if not notices:
            notices = upstream_notices(f'cargo:{name}@{version}', repository, vcs.get('git', {}).get('sha1'), vcs.get('path_in_vcs', ''))
        omitted_notice = False
        if not notices and name in {'fxhash', 'mac'}:
            omitted_notice = True
            apache = sorted(bases[0].glob('serde-*/LICENSE-APACHE'))[0]
            original = directory / ('lib.rs' if name == 'fxhash' else 'README.md')
            data = original.read_bytes().split(b'#!', 1)[0] if name == 'fxhash' else original.read_bytes()
            notices = [save_text(apache.read_bytes(), 'standard Apache-2.0 full text (selected declared alternative)', 'standard Apache-2.0 text; package license declared in Cargo.toml'),
                       save_text(data, original.name + ' original attribution/declaration', source),
                       save_text((directory / 'Cargo.toml').read_bytes(), 'Cargo.toml license/authors declaration', source)]
        entry = {'ecosystem': 'cargo', 'name': name, 'version': version,
                 'license': meta.get('license', 'UNSPECIFIED'), 'authors': meta.get('authors', []),
                 'repository': repository, 'source': source, 'source_sha256': package.get('checksum'),
                 'scope': 'locked union: runtime/build/dev/target-conditional; over-inclusive', 'notices': notices}
        if omitted_notice:
            entry['upstream_separate_notice_omitted'] = True
            entry['selected_license'] = 'Apache-2.0'
        # Supply unmodified covered MPL source, plus vendored libdbus AFL source.
        if 'MPL-2.0' in entry['license'] or name == 'libdbus-sys':
            archive = next((b / f'{name}-{version}.crate' for b in (cargo_home / 'registry/cache').glob('*') if (b / f'{name}-{version}.crate').exists()), None)
            if archive is None:
                raise RuntimeError(f'Missing MPL source archive {name}@{version}')
            data = archive.read_bytes()
            if sha(data) != package['checksum']:
                raise RuntimeError(f'Source checksum mismatch {name}@{version}')
            target = OUT / 'sources' / archive.name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            entry['covered_source'] = target.relative_to(ROOT).as_posix()
        entries.append(entry)
    return entries


def npm_lock_packages():
    # pnpm 12 can write multiple YAML documents (package manager + workspace).
    # Only read the top-level packages section, never peer snapshots/importers.
    packages = {}
    active = False
    current = None
    for line in (ROOT / 'pnpm-lock.yaml').read_text(encoding="utf-8").splitlines():
        if line == 'packages:':
            active = True
            continue
        if active and line and not line[0].isspace():
            active = False
        if not active:
            continue
        match = re.match(r"  ('[^']+'|[^ :]+):$", line)
        if match:
            key = match[1].strip("'")
            name, version = key.rsplit('@', 1)
            packages[(name, version)] = {}
            current = (name, version)
        elif current:
            integrity = re.search(r'integrity: ([^,}\s]+)', line)
            if integrity:
                packages[current]['integrity'] = integrity[1]
    if len(packages) < 100:
        raise RuntimeError('Unrecognized pnpm lock format')
    return packages


def installed_npm():
    result = {}
    for path in (ROOT / 'node_modules/.pnpm').glob('*/node_modules/*/package.json'):
        meta = json.loads(path.read_text(encoding="utf-8"))
        result[(meta['name'], meta['version'])] = (path.parent, meta)
    for path in (ROOT / 'node_modules/.pnpm').glob('*/node_modules/@*/*/package.json'):
        meta = json.loads(path.read_text(encoding="utf-8"))
        result[(meta['name'], meta['version'])] = (path.parent, meta)
    return result


def npm_inventory():
    installed = installed_npm()
    entries = []
    pending = []
    for (name, version), resolution in npm_lock_packages().items():
        key = f'npm:{name}@{version}'
        local = installed.get((name, version))
        if local:
            directory, meta = local
            source = f'https://registry.npmjs.org/{name}/-/{name.split("/")[-1]}-{version}.tgz'
            notices = local_notices(directory, source)
        elif key in PROVENANCE:
            meta = PROVENANCE[key]['metadata']
            notices = PROVENANCE[key]['notices']
        else:
            pending.append((name, version, resolution))
            continue
        entries.append(npm_entry(name, version, resolution, meta, notices))
    def metadata_task(item):
        name, version, resolution = item
        key = f'npm:{name}@{version}'
        meta = json.loads(fetch(f'https://registry.npmjs.org/{urllib.parse.quote(name, safe="@")}'+f'/{version}'))
        # Optional prebuilt tooling is not embedded in frontend. Notices from the
        # exact-version parent package apply to the same upstream project.
        family = None
        if name.startswith('@pnpm/exe.') or name == 'pnpm': family = 'pnpm'
        elif name.startswith('@esbuild/'): family = 'esbuild'
        elif name.startswith('@rollup/rollup-'): family = 'rollup'
        elif name.startswith('@tauri-apps/cli-'): family = '@tauri-apps/cli'
        elif name.startswith('@napi-rs/lzma-'): family = '@napi-rs/lzma'
        elif name.startswith('@tailwindcss/oxide-'): family = '@tailwindcss/oxide'
        elif name.startswith('lightningcss-'): family = 'lightningcss'
        elif name.startswith('@biomejs/cli-'): family = '@biomejs/biome'
        parent = installed.get((family, version)) if family else None
        notices = local_notices(parent[0], f'exact-version parent package {family}@{version}') if parent else []
        repository = meta.get('repository')
        if isinstance(repository, dict): repository = repository.get('url')
        if not notices:
            notices = upstream_notices(key + ':upstream', repository, meta.get('gitHead'))
        PROVENANCE[key] = {'metadata': {k: meta[k] for k in ['license', 'author', 'repository', 'gitHead'] if k in meta}, 'notices': notices}
        return npm_entry(name, version, resolution, meta, notices)
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for entry in pool.map(metadata_task, pending):
            entries.append(entry)
    # Installed packages without notices can use their repository at recorded VCS.
    for entry in entries:
        if not entry['notices']:
            local = installed.get((entry['name'], entry['version']))
            meta = local[1] if local else {}
            family = None
            if entry['name'].startswith('@esbuild/'): family = 'esbuild'
            elif entry['name'].startswith('@rollup/rollup-'): family = 'rollup'
            elif entry['name'].startswith('@tauri-apps/cli-'): family = '@tauri-apps/cli'
            elif entry['name'].startswith('@napi-rs/lzma-'): family = '@napi-rs/lzma'
            parent = installed.get((family, entry['version'])) if family else None
            entry['notices'] = local_notices(parent[0], f'exact-version parent package {family}@{entry["version"]}') if parent else []
            if not entry['notices']:
                entry['notices'] = upstream_notices(f'npm:{entry["name"]}@{entry["version"]}:upstream', entry['repository'], meta.get('gitHead'))
    for entry in entries:
        # An SPDX declaration preserves attribution but is not the full terms.
        # Resolve the exact registry version's VCS revision, never branch HEAD.
        if entry['notices'] and all(n['original_path'].lower().endswith('.spdx') for n in entry['notices']):
            key = f"npm:{entry['name']}@{entry['version']}"
            metadata_key = key + ':registry-metadata'
            metadata = PROVENANCE.get(metadata_key)
            if metadata is None:
                metadata = json.loads(fetch(f"https://registry.npmjs.org/{urllib.parse.quote(entry['name'], safe='@')}/{entry['version']}"))
                metadata = {k: metadata[k] for k in ['gitHead', 'license', 'repository'] if k in metadata}
                PROVENANCE[metadata_key] = metadata
            revision = metadata.get('gitHead')
            if not isinstance(revision, str) or not re.fullmatch(r'[0-9a-f]{40}', revision):
                raise RuntimeError(f"Cannot pin full upstream license terms for {key}")
            full_terms = upstream_notices(key + ':full-terms', entry['repository'], revision)
            if not full_terms or all(n['original_path'].lower().endswith('.spdx') for n in full_terms):
                raise RuntimeError(f"SPDX declaration is not full license terms for {key}")
            entry['notices'].extend(full_terms)
    for entry in entries:
        if not entry['notices'] and entry['name'] in {'@napi-rs/lzma-linux-x64-gnu', 'stackback'}:
            cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
            template_file = sorted((cargo_home / 'registry/src').glob('*/serde-*/LICENSE-MIT'))[0]
            template = template_file.read_text(encoding="utf-8")
            terms = template[template.index('Permission is hereby granted, free of charge'):]
            data = ('MIT full license terms as declared by upstream package metadata.\nThe upstream package/repository omits a separate license file. Original metadata\n(including author attribution where supplied) follows:\n\n' + json.dumps({k: entry.get(k) for k in ['name', 'version', 'license', 'authors', 'repository', 'source']}, indent=2) + '\n\n' + terms).encode()
            entry['notices'] = [save_text(data, 'metadata attribution plus standard MIT full terms', entry['source'])]
            entry['upstream_separate_notice_omitted'] = True
            entry['selected_license'] = 'MIT'
    return entries


def npm_entry(name, version, resolution, meta, notices):
    repository = meta.get('repository')
    if isinstance(repository, dict): repository = repository.get('url')
    return {'ecosystem': 'npm', 'name': name, 'version': version,
            'license': meta.get('license', 'UNSPECIFIED'), 'authors': meta.get('author', []),
            'repository': repository, 'source': f'https://registry.npmjs.org/{name}/-/{name.split("/")[-1]}-{version}.tgz',
            'source_integrity': resolution.get('integrity'),
            'scope': 'locked union: frontend/runtime/build/dev/optional tooling; over-inclusive', 'notices': notices}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    try:
        entries = cargo_inventory() + npm_inventory() + json.loads((ROOT / 'scripts/licenses/native-components.json').read_text(encoding="utf-8"))
    finally:
        CACHE.write_text(json.dumps(PROVENANCE, indent=2, sort_keys=True, ensure_ascii=False) + '\n', encoding="utf-8", newline="\n")
    for entry in entries:
        entry['notice_provenance'] = ('upstream_metadata+standard_terms' if entry.get('upstream_separate_notice_omitted') else 'upstream_file')
        if entry.get('upstream_separate_notice_omitted'):
            entry['notice_limitation'] = 'Published package and inspected upstream source omit a separate license file. Declared license alternative full terms and original metadata/source attributions are retained; this is not represented as an original upstream notice file. No copyright year or additional author has been invented.'
    entries.sort(key=lambda e: (e['ecosystem'], e['name'], e['version']))
    missing = [f'{e["ecosystem"]}:{e["name"]}@{e["version"]}' for e in entries if not e['notices']]
    inventory = {'schema_version': 1, 'scope': 'Complete Cargo.lock and pnpm-lock.yaml package union; includes dependencies not shipped on every target.',
                 'lockfiles': {p: sha((ROOT / p).read_bytes()) for p in ['Cargo.lock', 'pnpm-lock.yaml']},
                 'missing_notices': missing, 'upstream_notice_omissions': [e['ecosystem'] + ':' + e['name'] + '@' + e['version'] for e in entries if e.get('upstream_separate_notice_omitted')], 'dependencies': entries}
    (ROOT / 'docs/dependency-licenses.json').write_text(json.dumps(inventory, indent=2, ensure_ascii=False) + '\n', encoding="utf-8", newline="\n")
    biome = ROOT / 'node_modules/.bin/biome'
    if biome.is_file():
        subprocess.run([str(biome), 'check', '--write', str(ROOT / 'docs/dependency-licenses.json'), str(CACHE)], check=True, cwd=ROOT)
    print(f'{len(entries)} packages; {len(missing)} without full license terms; {len(inventory["upstream_notice_omissions"])} upstream separate-notice omissions explicitly recorded')
    for item in missing: print(item)
    return bool(missing)


if __name__ == '__main__':
    sys.exit(main())
