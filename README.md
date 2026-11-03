# MediaAtlas

Windows 优先、兼容 macOS/Linux 的本地媒体管理桌面应用。采用 Tauri 2、React、TypeScript 和 Rust。

当前实现 **Phase 6 独立 Music 工作流**：Artist / ReleaseGroup / Release / Medium / Track / Recording 独立存储，MusicBrainz 与 Cover Art Archive 真实请求适配，明确区分发行版本、地区、重制/豪华版、多碟及日期精度。Music Library 只读展示刮削成果，点击本地音频可请求系统默认应用打开；独立 Music 工具区承载搜索、版本导入、文件关联及标签 Diff / 明确确认 / 原字节备份 / 写入 / 恢复。保留 P4 Movie/TV/基础 Anime 与 P5 成人隔离工作流。Provider 默认禁用，实时站点、系统默认应用与 Windows/macOS 原生运行仍 pending；当前不是已通过全平台验收的发布版本。

在 **Settings → Provider 认证** 手动保存 TMDB v4 Bearer/v3 API Key；成人区解锁后可设置 FANZA/DMM 的 API ID + affiliate ID。默认系统密钥服务，不可用明确失败；可显式选择退出即失效的会话存储，秘密从不回显或进入 DB/日志/localStorage。保存认证与启用来源分开。AniList、Wikipedia 和 Wikidata 使用公共读取访问。

成人入口默认隐藏，可在 Settings 保存“始终显示成人媒体入口”。该选项重启后仍保留，锁定时入口继续显示，内容需重新解锁；封面与最近访问偏好单独设置。普通海报墙、统计、查询、任务及 Network Inspector 始终排除成人数据。UI 仅 `zh-CN` / `en-US`，网页预览需要桌面后端，详情见 [P5 与视频 MVP 验收](docs/acceptance/phase-5.md)。

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

默认使用本机应用数据目录中的 `library.sqlite`；扫描阶段只读；视频输出仅在用户明确确认后创建 NFO/海报 sidecar，已有文件保持原样。P6 音频标签修改和恢复各自要求 Preview 与明确确认，备份原字节保留，外部改动或来源关联变化时拒绝旧计划。海报使用本地缓存缩略图，远程图片仅由受控后端按所选候选获取。成人入口默认隐藏，开启后为独立分区；关闭会清除前端查询缓存。语言选择持久化，默认 `zh-CN`，缺失翻译回退 `en-US`。

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

Movie/TV/Anime/JAV/Music 类型扩展已完成独立结构及序列化，具体在线业务按阶段追加。P2 支持目录扫描任务的后台执行、取消和遗留任务标记 Interrupted；等待确认的候选保留，运行中任务不会自动重放。P4 已提供普通视频在线适配与本地 Mock 全闭环；P6 已提供独立音乐图谱与可恢复音频标签修改；文件移动与 Watcher 留待后续阶段。Windows 输出使用限时源文件/目录句柄，重启或预览过期后需要重新生成预览，实际 Windows 运行仍待验收。

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

本会话按用户授权逐阶段实施、验收并以 Codex 身份提交/推送 P3→P4→P5→视频 MVP→P6→P7→P8；未验收项保留 pending，不自动创建 tag 或发布。

## Provider 开发与验收

在桌面 Providers 页显式启用 Offline Fixture 后运行；Task Center 与 Network Inspector 可查看持久化任务和脱敏记录。Fixture 不联网、不写媒体；P4/P5已接入真实请求代码，live授权与可用性独立验收。网络引擎和Adapter可脱离Tauri测试，常规CI仅依赖Mock HTTP和Fixtures。详见 [Provider契约](docs/provider-contract.md) 与 [Phase 3验收](docs/acceptance/phase-3.md)。

P4 契约与边界：[phase-4-contract](docs/phase-4-contract.md)、[Provider 访问条件](docs/provider-access.md)、[安全输出](docs/media-output.md)、[阶段验收](docs/acceptance/phase-4.md)。系统密钥服务真实读写、实时授权及原生/跨平台 GUI 验收不由 Mock 测试替代。

P5 契约与边界：[phase-5-contract](docs/phase-5-contract.md)、[P5/视频 MVP 验收](docs/acceptance/phase-5.md)。成人通道的 Preview/Confirm/批量取消不放宽普通输出隔离，测试只操作临时合成媒体。

P6 契约与验证：[Music 契约](docs/phase-6-contract.md)、[P6 验收](docs/acceptance/phase-6.md)。音乐 Library 仅供浏览与系统应用打开；刮削、标签编辑和恢复在独立工具区。旧影视/成人详情工作流的同类 UI 分离列入后续计划，不因 P6 视作已完成。
