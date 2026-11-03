#!/usr/bin/env python3
"""Verify the release's native-linking assumptions against Cargo's selected graph.

Does not replace actual release ELF/package inspection. Linux binaries must also
use system WebKit/GTK/libdbus and packages must not contain extra shared libraries.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def cargo_tree(target, *arguments):
    result = subprocess.run(['cargo', 'tree', '--workspace', '--locked', '--target', target, *arguments], cwd=ROOT, text=True, encoding="utf-8", capture_output=True)
    if result.returncode:
        raise ValueError(result.stderr.strip())
    return result.stdout.replace(str(ROOT), '.')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--target', default='x86_64-unknown-linux-gnu')
    parser.add_argument('--write-evidence', action='store_true')
    args = parser.parse_args()
    selected = cargo_tree(args.target, '-e', 'normal,build', '--prefix', 'none')
    if re.search(r'^openssl-sys v', selected, re.M):
        raise ValueError('Unexpected selected OpenSSL native dependency; reassess redistribution materials')
    evidence = {'target': args.target, 'selected_openssl_sys': False,
                'method': 'cargo tree --workspace --locked --target TARGET, selected normal/build edges plus inverted libdbus features. Static feature evidence only; actual executable and package inspection are separate.'}
    if 'linux' in args.target:
        dbus = cargo_tree(args.target, '-e', 'features', '-i', 'libdbus-sys')
        if not dbus.strip() or 'feature "vendored"' in dbus or 'feature "cc"' in dbus:
            raise ValueError('Release requires dynamic system libdbus; vendored/static libdbus or missing expected dbus graph')
        if 'feature "pkg-config"' not in dbus:
            raise ValueError('Expected libdbus system pkg-config selection')
        evidence['libdbus_features'] = dbus
        evidence['libdbus_linkage'] = 'System pkg-config, not vendored. Confirm DT_NEEDED libdbus-1.so.3 and package depends libdbus-1-3 on the actual release.'
    if args.write_evidence:
        destination = ROOT / 'scripts/licenses/native-audit.json'
        current = json.loads(destination.read_text(encoding="utf-8")) if destination.exists() else {}
        current['current_release_graph'] = evidence
        current['features']['keyring'] = 'sync-secret-service + crypto-rust; vendored feature removed'
        current['features']['libdbus-sys'] = 'dynamic system pkg-config; not bundled in current release'
        current['libdbus']['scope'] = 'The vendored 1.14.4 source/headers below are retained as over-inclusive historical/optional source materials. They are not compiled into the current release.'
        destination.write_text(json.dumps(current, indent=2) + '\n', encoding="utf-8", newline="\n")
    print(f'Native feature policy verified for {args.target}: system libdbus on Linux, no selected openssl-sys. Actual package check still required.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError) as error:
        print(f'Native feature verification failed: {error}', file=sys.stderr)
        sys.exit(1)
