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
