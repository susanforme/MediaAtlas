# MediaAtlas

Windows 优先、兼容 macOS/Linux 的本地媒体管理桌面应用。采用 Tauri 2、React、TypeScript 和 Rust。

当前实现 **Phase 3 双引擎与 Provider 生命周期**（基于 Phase 2 本地扫描与媒体库）：添加/管理 Root、后台手动及增量扫描、分页候选预览、勾选确认入库、任务进度/取消，以及真实本地海报墙、列表、筛选和详情。只读本地 NFO/音频标签，新增 Provider 注册、API/HTML 双引擎、可取消任务与脱敏网络记录；当前只内置显式禁用的离线 Fixture，不接实时站点、不移动或改写源媒体。UI 暂时仅支持 `zh-CN`、`en-US`；浏览器预览明确提示需要桌面运行。

## 开发环境

- Node.js 24 LTS（前端最低 22.12）。
- **pnpm 12.17.0**：本轮查询到的最新稳定 12.x，固定在 `package.json`；CI 读取同一版本。升级后同步锁文件。
- Rust **1.90.0**，由 `rust-toolchain.toml` 固定，含 rustfmt 和 clippy。
- [Tauri 2 平台前置依赖](https://v2.tauri.app/start/prerequisites/)：Windows 需要 MSVC C++ Build Tools 和 WebView2；macOS 需要 Xcode Command Line Tools。
- Ubuntu/Debian 还需 GTK/WebKit 开发包，例如：

```bash
sudo apt-get update
sudo apt-get install -y build-essential libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

准备好 Node 后，可使用支持的包管理器安装固定版本：

```bash
npm install --global pnpm@12.17.0
pnpm install --frozen-lockfile
```

## 启动与构建

在仓库根目录执行：

```bash
pnpm dev             # Tauri 桌面开发，自动启动 Vite :1420
pnpm dev:web         # 仅网页预览，不能验证桌面 IPC
pnpm build           # 前端生产构建
pnpm build:desktop   # 本平台桌面可执行文件，不生成安装包、不发布
```

桌面启动后在 Root 页面输入本地目录、名称和媒体类型，再进入扫描页。扫描完成先显示候选，只有勾选并再次确认的条目才进入媒体库。诊断页保留本地核心连接检查和真实版本；网页模式无法调用后端，不伪造媒体记录。

默认使用本机应用数据目录中的 `library.sqlite`；原媒体目录只读。海报来自本地文件，在应用缓存中生成小图。成人入口默认隐藏，开启后为独立分区；关闭会清除前端查询缓存。语言选择持久化，默认 `zh-CN`，缺失翻译回退 `en-US`。

## 检查

```bash
pnpm check                             # Biome、TypeScript、Vitest、前端构建
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

默认 Rust 成员为四个 core crate 和独立只读 scanner，可在未安装 GTK/WebKit 的环境独立测试。完成前端构建、安装平台依赖后，检查完整桌面 workspace：

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## CI 与 tag 发布

- `.github/workflows/ci.yml`：`main` push、PR 和手动运行仅在 Windows/macOS/Linux 执行 `pnpm test` 与 `cargo test --locked`；Rust 默认成员包含 SQLite 测试，不构建 Tauri 桌面、不安装 GTK、不打包或发布。
- `.github/workflows/release.yml`：只由 push tag 触发。分别构建 Windows NSIS、macOS DMG 和 Linux AppImage/deb，所有平台打包成功后才创建/更新对应 GitHub Release 并上传产物。不由普通分支 push、PR 或手动按钮触发，也不自行创建 tag。
- `.github/tauri.release.conf.json` 只在 tag 构建时启用打包；本地 `pnpm build:desktop` 仍只生成可执行文件。
- tag 名称不会自动修改应用版本，发布前需同步 Cargo workspace、Tauri 和前端包的版本。当前工作流未接入签名凭据或自动更新发布，完整发布验收仍属 P8。

工作流配置及静态检查不等于远端 CI/安装包已验证；Phase 0 平台启动待验收项仍保留在记录中。

## Phase 1 本地存储示例

```bash
cargo run -p mediaatlas-infrastructure --example library_roundtrip --locked
```

该示例只在临时目录创建 Fixture SQLite，验证 Root/媒体/文件索引、按 Root 分页、人工字段保护及关闭重开；不会扫描、移动或写入真实媒体。P2 桌面启动会打开应用自身的 SQLite，但不会自动扫描用户媒体目录。

Movie/TV/Anime/JAV/Music 类型扩展已完成独立结构及序列化，具体在线业务按阶段追加。P2 支持目录扫描任务的后台执行、取消和遗留任务标记 Interrupted；等待确认的候选保留，运行中任务不会自动重放。文件移动、标签写入、在线刮削与 Watcher 均未实现。

## 目录与契约

```text
apps/desktop/src/             React、路由、i18n、typed IPC
apps/desktop/src-tauri/       Tauri 宿主、窗口、command/event、最小权限
crates/domain/               公共实体、独立媒体模型、路径与元数据验证
crates/application/          应用信息用例
crates/desktop-bridge/       不依赖 Tauri 的 DTO 和错误协议
crates/infrastructure/       结构化日志、SQLite Repository、追加迁移
crates/media-scanner/        只读遍历、分类型文件名解析、本地 NFO/音频标签
docs/                        基线、架构决策、任务与验收
```

P0 `get_app_info` 契约不变；P2 新增 Root/浏览/扫描命令，返回 `{schema_version:1,data:T}`。任务事件用于刷新提示，持久化查询和轮询为权威来源，UI 不依赖事件必达。完整契约见 [Phase 2 接口](docs/phase-2-contract.md)。前端不访问刮削站点。

日志写入 Tauri `app_log_dir`，每日滚动、最多保留 7 份，并向 stderr 输出 JSON。P0 日志只记录固定启动信息，不接收用户 payload；网络和文件业务的脱敏在对应阶段落实。

- [原始实施基线](docs/UNIVERSAL_MEDIA_SCRAPER_AGENT_PLAN.md)
- [阶段任务](docs/agent-plan.md)
- [架构与 IPC](docs/architecture.md)
- [初始仓库审计](docs/repo-audit.md)
- [Phase 0 验收记录](docs/acceptance/phase-0.md)
- [领域模型](docs/domain-model.md)
- [数据库 Schema](docs/database-schema.md)
- [Phase 1 验收记录](docs/acceptance/phase-1.md)
- [扫描工作流](docs/library-workflow.md)
- [界面规范](docs/ui-spec.md)
- [Phase 2 验收记录](docs/acceptance/phase-2.md)

本轮提交 Phase 2 后停止，等待用户明确指示才开始 Phase 3。

## Provider 开发与验收

在桌面 Providers 页显式启用 Offline Fixture 后运行；Task Center 与 Network Inspector 可查看持久化任务和脱敏记录。Fixture 不联网、不写媒体，真实Provider留待P4。网络引擎和Adapter可脱离Tauri测试，常规CI仅依赖Mock HTTP和Fixtures。详见 [Provider契约](docs/provider-contract.md) 与 [Phase 3验收](docs/acceptance/phase-3.md)。
