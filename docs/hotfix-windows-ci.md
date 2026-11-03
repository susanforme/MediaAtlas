# Windows CI hotfix after Phase 7

The failing run is [41607055843](https://github.com/susanforme/MediaAtlas/actions/runs/41607055843), commit `8d55797e7df353b6afac958d3b28e8534f96c2cc`. The supplied Windows log identifies the Rust core/database step, [job 125819678457, step 9](https://github.com/susanforme/MediaAtlas/actions/runs/41607055843/job/125819678457#step:9:520). The frontend step passed.

## Confirmed causes

- Adult scraper (`adult_scraper.rs:1055`), organization (`organization/tests.rs:78`) and video scraper (`scraper.rs:905`) fixtures registered actual Windows temporary directories with `PathStyle::Posix`. `LibraryRepository::save_root` correctly rejected those paths with `InvalidInput`. Fixtures now use the native path style. Shared filesystem fixtures also create temporary directories below the canonical temporary parent, matching the production registration contract and avoiding macOS `/var` aliases. Windows extended disk/UNC prefixes follow the production conversion. No production descendant no-follow protection is relaxed.
- `MediaOutputService::execute_cancellable` removed the Windows source lease after every result, including another channel's rejected request, a sidecar collision, and temporarily disabled/isolated roots. Later valid retries failed with `Conflict`. Successful execution releases its lease; rejected operations retain it until explicit discard, privacy revocation, bounded expiry or a successful retry. Lease lookup also checks its original adult grant. A new regression rejects ordinary execute/discard of an adult preview and then executes that preview successfully through its authorized service.
- A completed output whose source was subsequently removed returned `InvalidInput` during replay. It now returns `Conflict`, because the immutable completed plan is stale. Missing sources of unexecuted plans still use their existing rejection behavior.

The direct adult-grant test now obtains a fresh preview after revocation, consistent with the existing privacy-lock contract. Existing lock/unlock tests still reject old previews; no test is skipped and source-preservation assertions remain.

## Validation

The patch is tested from a separate export of `8d55797` containing only these changes, preserving its original versions and lockfiles and excluding Phase 8 work. Linux validation:

- `pnpm install --frozen-lockfile`, then `pnpm check`: lint, typecheck, all 150 frontend tests and the production web build pass.
- `cargo fmt --all -- --check`: passes.
- `cargo test --workspace --locked --offline`: all 264 tests pass, including 171 infrastructure unit tests.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passes without warnings.

Logs: `/tmp/hotfix-isolated-pnpm.log`, `/tmp/hotfix-isolated-fmt.log`, `/tmp/hotfix-isolated-tests.log`, `/tmp/hotfix-isolated-clippy.log` in the execution workspace. A passing Linux run and inspection of the actual Windows dependency APIs do **not** establish a passing native Windows/macOS run. Native acceptance remains pending the new GitHub Actions run after push.

The fixture regressions include an actual symlink alias for a temporary parent. Existing source/ancestor substitution, no-clobber output, root privacy, failed-output retry and Windows no-delete/discard assertions remain enabled on their applicable platforms.

## macOS follow-up

Native run [41609118246](https://github.com/susanforme/MediaAtlas/actions/runs/41609118246) confirms Windows job `125826443115` and Linux job `125826443057` succeeded for `55da914`. macOS job [`125826442718`](https://github.com/susanforme/MediaAtlas/actions/runs/41609118246/job/125826442718) passed frontend tests; the supplied Rust log reports 168 passing tests and exactly two failures: the CAA fixture below and the watcher epoch test at `watcher/tests.rs:228` (`left: 1`, `right: 0`).

A remaining affected fixture was independently reproduced: `music_providers::tests::mock_caa_download_reuses_bounded_hash_cache_and_ignores_arbitrary_remote_image_url` constructs `MediaOutputService` beneath a raw `tempfile::tempdir`. Setting `TMPDIR` to a real directory symlink reproduces the macOS temporary-parent alias and makes the test fail at `music_providers.rs:919` with `InvalidConfiguration`. The production cache correctly rejects that ancestor symlink. Both music-provider fixtures now reuse the canonical temporary-directory helper. The same aliased-parent run then passes all four music-provider tests. Logs are `/tmp/macos-alias-binary-before.log` and `/tmp/macos-alias-after.log`.

The watcher test deliberately injects old queue/overflow generations, but also attached real OS callbacks while creating and watching a new directory. A valid new-generation FSEvents notification could therefore increase its global task count; that counter did not identify the injected signal as its cause. A per-runtime gate compiled only for unit tests now isolates native callback delivery in this one injection test. Its zero-task assertions remain unchanged, and a new positive control injects the fresh generation, awaits the real worker and verifies the saved scan snapshot names the new root. The test reloads the persisted root before its disabled-watch assertion, matching the database timestamp precision. All real native-event integration tests leave callbacks enabled. Production callback behavior, generation filtering, overflow filtering and transactional expected-root checks are unchanged.

The CAA change touches fixture construction only. It preserves the production cache's no-follow behavior and every existing assertion. Native macOS acceptance still requires a successful subsequent CI run and review of any remaining failure summary.

Final follow-up validation uses an isolated export of `55da914` with only these follow-up files: all 264 Rust workspace tests pass with `TMPDIR` set to the real symlink alias, workspace formatting passes, and full workspace/all-target Clippy passes with warnings denied. Logs are `/tmp/macos-hotfix-isolated-tests.log`, `/tmp/macos-hotfix-isolated-fmt.log` and `/tmp/macos-hotfix-isolated-clippy.log`. The 10 watcher regressions also pass independently (`/tmp/macos-watcher-after.log`), including the real native-event tests with their callback gates left enabled. Frontend source is unchanged from the native CI's passing frontend step.

## Root replacement follow-up

Run [41611303933](https://github.com/susanforme/MediaAtlas/actions/runs/41611303933) for `dbea7e8` passes native Windows and Linux checks. Its macOS job [`125833580657`](https://github.com/susanforme/MediaAtlas/actions/runs/41611303933/job/125833580657) reports 169 passing Rust unit tests and one failure: `replaced_or_missing_filesystem_root_retires_native_generation_and_can_recover`, at `watcher/tests.rs:412`, expected zero tasks but observed one. The previous two failures passed.

This root-identity test had the same mixed event sources: after renaming away the old root, it created and watched a replacement directory, injected an old generation and then counted every task. It now uses controlled notification delivery before installing any watcher. The original zero-task assertion, inactive-while-missing assertion and distinct-generation assertion remain. A positive control sends the new generation through the actual queue/scan pipeline and checks that the resulting preview contains only `Recovered.2024.mkv` from the replacement directory, while the renamed old root's sentinel bytes remain intact.

The complete watcher test audit uses the explicit control helper only for manually injected epoch, overflow, shutdown-acknowledgement and root-identity tests. Real native debounce/restart, private-path filtering and offline catch-up tests retain OS callback delivery. Production watcher code is unchanged, no test is skipped and no timing allowance is increased. The local 10-test watcher run passes with the aliased temporary parent (`/tmp/macos-root-epoch-watcher.log`); native macOS success still requires the next CI result.

An isolated export of `dbea7e8` plus this test-only patch passes all 264 Rust workspace tests with the symlink-aliased `TMPDIR`, workspace formatting and workspace/all-target Clippy with warnings denied. Logs: `/tmp/macos-root-isolated-tests.log`, `/tmp/macos-root-isolated-fmt.log`, `/tmp/macos-root-isolated-clippy.log`. Frontend code remains unchanged from the passing native CI checks.
