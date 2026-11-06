# Tagged publication

`release.py` checks versions, inspects native payloads, stages each target and aggregates the complete required target set. `publish.py` performs the final publication gates and supports rebuilding a tag that already has a Release.

Push `vX.Y.Z` for a stable release or `vX.Y.Z-beta.N` for a prerelease, with positive beta numbers and no leading zeros in numeric components. The workflow accepts `v*` tags, then preflight rejects unsupported tags and requires the version to match every project manifest and workspace entry in Cargo.lock. Alpha, RC and build-metadata tags are not supported. Stable releases set `prerelease=false` and use GitHub's `make_latest=legacy` selection by release date and semantic version; beta releases set `prerelease=true` and `make_latest=false`. Both stay drafts until all packages and remote assets pass verification.

Updating workspace versions also changes Cargo.lock. Refresh its SHA-256 in `docs/dependency-licenses.json` even when third-party dependencies are unchanged; the release verifier still checks the exact lockfile inventory and all original license materials.

The workflow checks out its event commit, cancels older runs in the same repository/tag concurrency group, and resolves the live GitHub tag before scheduling builds. Annotated tags are dereferenced to their actual commit. The publisher repeats that check before each release mutation and before publishing. A run whose tag moved stops without continuing uploads or publication.

Before changing GitHub, `publish.py publish` regenerates the aggregate from all verified target directories and compares the complete output set byte for byte, including manifests, notices and checksums. A changed file, missing required platform, unknown asset, stale provenance or failed test record blocks publication.

For an existing Release, the publisher reuses its ID and first changes it to a draft. If the published-by-tag endpoint returns 404, it searches all visible release pages for the same tag, including a draft left by an earlier failed rebuild; multiple matches are rejected. All subsequent ownership checks use the release ID and verify its tag name. It replaces every expected asset, then checks the exact remote filename set, upload state, size and GitHub SHA-256 digest before making the Release public again. GitHub has no atomic multi-asset replacement API; hiding the Release during replacement prevents publishing mixed package generations. Failed uploads or remote verification leave a draft that a later successful run can rebuild. Unexpected existing assets fail closed and are never automatically deleted. The script does not move tags or delete Releases.

Local checks (no release mutation):

```sh
python -m unittest discover -s scripts/release -p 'test_*.py' -v
python scripts/release/publish.py check-current --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --repository "$RELEASE_REPOSITORY"
```

The workflow alone invokes the mutating `publish` command after its complete native build matrix and artifact checks. Development verification does not execute that command against GitHub.

The workspace release profile targets code size with `opt-level="s"`, thin LTO, one codegen unit and stripped symbols. Compilation/linking can take longer, and native backtraces may lose function symbols. Panic unwinding remains enabled. Package-size savings are not asserted before the tagged native builds produce and verify actual assets.

The release-only Tauri overlay enables [`build.removeUnusedCommands`](https://v2.tauri.app/concept/size/#remove-unused-commands) to omit framework/plugin IPC commands that the static capability files never allow. Those files already list explicit event, window and dialog permissions; their allowed commands remain available. Application commands remain registered without an application ACL. Reevaluate this option before adding runtime capabilities, a custom capabilities path or an application ACL: runtime-added permissions are not included in command pruning. This changes neither dependency features nor the panic strategy; additional size savings remain unmeasured until native release builds.

Windows NSIS packages use LZMA compression, Tauri's default NSIS codec, for a smaller installer. See the [Tauri configuration schema](https://schema.tauri.app/config/2) and [NSIS compression reference](https://nsis.sourceforge.io/Reference/SetCompressor). The release gate still extracts the actual installer with 7-Zip and checks its executable architecture, CRT imports, product version and the absence of license resource files. It never treats an extraction failure as a successful verification or executes the installer as a fallback. The earlier BZip2 choice and zlib extraction failure remain historical evidence in the hotfix notes.

`test_nsis.py` compiles a file-only installer without notice materials and verifies a complete extraction when `makensis` and `7z`/`7zz` are available. Explicit tool paths can be supplied with `MEDIAATLAS_TEST_MAKENSIS` and `MEDIAATLAS_TEST_7Z`; an extracted NSIS toolset may also need its documented `NSISDIR` variable. The installer is not executed. See [Windows hotfix evidence](../../docs/hotfix-windows-ci.md#nsis-package-extraction-2026-11-03).

Windows portable ZIPs contain exactly `MediaAtlas.exe`; NSIS remains a separate asset. ZIPs use standard Deflate at compression level 9. All platforms omit license materials from both installed resources and executables. The complete materials remain in the standalone Release attachments, including `license-materials.zip`, and keep their source, content and SHA-256 checks. The application has no license-export command. Normal startup and single-file NFO association remain available; other arguments exit before GUI/database startup.

`stage` verifies the ZIP's single member, CRC, real native architecture, PE CRT imports and equality to the compiled EXE hash without executing the payload. Native package extraction rejects license resource files, and every app binary is checked for the removed embedded notice inventory marker. The aggregate rejects target manifests that do not record this exclusion, while continuing to generate the complete external license archive. No PE overlay is appended and extraction verification is not bypassed.

`test_portable.py` checks the resource-free package configuration, ZIP member/CRC/hash rules and preservation of native checks without executing the app. Main CI runs it on Windows, macOS and Linux. These verifier regressions do not claim an installed application or a smaller measured package; the new tagged native builds establish actual package sizes.
