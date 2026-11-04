# Windows CI hotfix after Phase 7

## NSIS package extraction (2026-11-03)

Run [41644340353](https://github.com/susanforme/MediaAtlas/actions/runs/41644340353), Windows job [125945685669](https://github.com/susanforme/MediaAtlas/actions/runs/41644340353/job/125945685669), successfully builds the tagged Rust application and a 13.75 MiB NSIS installer. The subsequent `release.py stage` fails when 7-Zip opens that installer as NSIS. This is package inspection, not a compiler failure. The failed job did not upload its installer, so the exact CI archive was unavailable for local byte-level inspection.

The compatibility issue was reproduced locally with the exact upstream [NSIS v3.13 source](https://github.com/NSIS-Dev/nsis/tree/a4ba28e60bd9e7c271cd2bfc1558889ce3355653), its [Tauri-distributed 3.13 stubs](https://github.com/tauri-apps/binary-releases/releases/tag/nsis-3.13), and 7-Zip 25.01. A file-only fixture includes the repository's complete license resource tree and a sentinel payload. With solid zlib compression it produces the same `Cannot open the file as [Nsis] archive` failure; with the same files and solid BZip2 or LZMA it extracts all 559 files successfully. The extracted archive header is 109,452 bytes. A small zlib fixture does extract, so a tiny probe alone would miss this resource-layout-sensitive failure. The [7-Zip NSIS reader](https://github.com/ip7z/7zip/blob/main/CPP/7zip/Archive/Nsis/NsisIn.cpp) selects its legacy NSIS Deflate decoder and notes missing NSIS3 detection; this is consistent with the reproduction, rather than proof that the unavailable CI bytes are corrupt.

The release config now uses BZip2, whose full terms are already included in `NSIS-COPYING.txt`. Native inventory scope text is synchronized; no dependency or bundled license text is replaced. The actual installer extraction, PE architecture, CRT imports, version and complete notice verification remain mandatory. Failed subprocess stdout is preserved so future logs include extractor version and format diagnostics. There is no installer-execution fallback or verification bypass.

The new real-resource roundtrip regression uses the configured codec, compares the sentinel bytes, checks the exact extracted file set, and runs the existing complete license verifier. It is exercised locally with explicitly supplied native tools; environments without those tools report that optional integration test as unavailable. Native Windows package acceptance still requires a new release workflow built from the fix. Re-running a job pinned to the old tag commit would not use this change; no tag is moved by this repair.

Local validation passes 48 release tests (including the actual NSIS roundtrip, not skipped), 18 license tests and 7 CI tooling tests. The license verifier confirms 1,001 locked dependencies and 1,625 notice references; beta version preflight and diff whitespace checks pass. Build/extraction reproduction logs and tooling remain in ignored `work/nsis-investigation/`, with no generated installer committed or executed.

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

## File identity write contention

Native macOS run [41620630364](https://github.com/susanforme/MediaAtlas/actions/runs/41620630364), job [`125865114510`](https://github.com/susanforme/MediaAtlas/actions/runs/41620630364/job/125865114510), failed `child_identity_cannot_be_reassigned_and_parent_updates_do_not_replace_rows` at `library_repository.rs:518` on `d7e190a`. This is the second rejected reassignment of the same file, after rejecting a different media owner. Its fixture uses fixed millisecond timestamps and logical database paths; filesystem aliases and migration line endings are not involved.

`save_file` previously began a deferred transaction, read the root, then attempted its UPSERT. A different connection still holding the writer can make that read-to-write upgrade fail immediately with SQLite busy rather than wait for the configured timeout. A rejected UPSERT also returned through `?`, leaving SQLx to queue rollback on drop. The following request could therefore encounter that pending writer. New controlled regressions reproduce `Err(Database)` before the fix, both with a deliberately held SQLite writer and with 24 concurrent rejected identity changes (`/tmp/save-file-writer-before.log`). The public CI annotation's `Conflict` category alone does not reveal the actual left-hand assertion value.

`save_file` now starts `BEGIN IMMEDIATE` through SQLx's transaction API before reading the root. Pre-commit validation and query errors explicitly await rollback; successful writes commit normally. The original UPSERT identity conditions, error mapping, normalized-path uniqueness and parent/child preservation assertions are unchanged. No Root row is updated just to acquire the writer, and no retries, sleeps or relaxed assertions were added to production.

The held-writer regression checks that the operation remains pending while the real reservation exists, releases that reservation, then verifies the original `Conflict` and unchanged file. The concurrent regression checks all 24 owner/root reassignments are rejected, and that 24 distinct file IDs competing for one path produce exactly one success and 23 conflicts. All 13 repository tests pass after the fix (`/tmp/save-file-writer-after.log`). The 100 ms bounded pending check verifies contention behavior; it is not a delay used to repair ordering. Native macOS confirmation still requires the next CI result.

Final Linux validation passes all 305 Rust workspace tests, workspace formatting and workspace/all-target Clippy with warnings denied. Logs are `/tmp/save-file-final-workspace.log`, `/tmp/save-file-final-fmt.log` and `/tmp/save-file-final-clippy.log`. Frontend source, beta.2 version and lockfiles are unchanged.

## 刮削测试的文件身份与取消同步（2026-11-04）

独立修复提交 `127f49c`。成人自动测试夹具之前将文件名原始大小写放进索引键；Windows 仓库保存规范化键后，最终守卫无法匹配而返回 Conflict。夹具改为读取持久化的 normalized identity，并新增“规范化键可用、其他文件键仍拒绝”的回归；生产守卫未放宽。

成人详情撤权与并发图库测试改用实际请求进入/响应 gate 同步。图库并发用例明确配置两个图片并发槽，不再依赖固定 25ms 延时；成人响应保持到取消实际完成，避免撤权轮询与立即响应之间的测试竞争。

[CI run 41757918195](https://github.com/susanforme/MediaAtlas/actions/runs/41757918195) 的 Windows、macOS、Linux 全部成功。Windows [job 126338312682](https://github.com/susanforme/MediaAtlas/actions/runs/41757918195/job/126338312682) 于 2026-11-04 10:08:51（北京时间）完成 Rust 核心步骤；原三个失败测试及新增规范化身份测试均通过，infrastructure 单元 214 通过、0 失败。此证据属于独立修复，不代替后续 P0 队列版本的跨平台 CI。

## Provider 取消与禁用的测试时序（2026-11-04）

[beta.5 main CI 41765406478 的 Windows job](https://github.com/susanforme/MediaAtlas/actions/runs/41765406478/job/126362795209) 在 `provider_runtime` 的 `cancel_and_disable_are_terminal_and_do_not_overwrite_success` 失败：任务终态为取消，`snapshot.requests.is_empty()` 却不成立。

旧用例把默认 250ms 的离线 fixture 延迟当作同步保证。Windows DB 操作可能晚于解析完成、早于结果事务提交；终态取消可以保持正确，已实际发生的 FIXTURE 解析审计记录仍应保留。这些不是取消后新发出的 HTTP 请求，不能通过清除真实审计记录满足测试。

用例改为受控 adapter：明确等待任务进入执行，成功路径由零 permit 的 Semaphore 阻塞；取消第二项、禁用第一项后分别等待取消退出事件，再断言两个终态和请求记录为空。第三项仅在显式 permit 后成功，再取消及禁用仍保持成功、无 failure，历史恰有该项两条记录。移除原 300ms sleep，生产实现保持不变。

本地 `cargo test --locked --offline -p mediaatlas-infrastructure --test provider_runtime`：7/7；关键用例独立重复 20 次全通过，Rustfmt 与 diff 检查通过。最终 Windows 结果以包含本改动的新 main CI 为准。修复随 beta.6 新 tag 发布，不移动 beta.5。
