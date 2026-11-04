# MediaAtlas

Windows 优先、兼容 macOS/Linux 的本地媒体管理桌面应用。采用 Tauri 2、React、TypeScript 和 Rust，复用系统 WebView，不捆绑 Chromium：Windows 使用系统 WebView2（安装器可下载官方 bootstrapper），macOS 使用系统 WKWebView，Linux deb 依赖系统 WebKitGTK/GTK。

main 默认进入媒体库海报墙：添加目录后后台扫描、建立本地索引并补全可靠匹配的资料，缺资料时仍可浏览。海报进入详情，系统应用打开是明确操作，多版本/Part 需要选择。来源未配置、歧义和失败通过实际状态处理；磁盘旁车、标签与组织操作仍独立预览和确认。流程基准见 [Emby 与成熟刮削工具流程](docs/emby-scraper-workflow.md)及[自动媒体库](docs/automatic-library.md)。

在“设置 → 数据来源”手动保存认证并明确启用来源。TMDB 支持 v4 Bearer / v3 API Key；FANZA/DMM 使用 API ID + affiliate ID。默认使用系统密钥服务；也可显式选择退出即失效的会话存储。秘密不回显、不写入数据库/日志/localStorage，不从运行环境自动配置。保存凭据、启用来源与更新媒体库是独立动作。

成人资源类型默认关闭，启用后复用统一浏览、详情与工具，应用建立后台会话授权，无额外解锁页面。普通“全部”、统计、查询与任务保持隔离，关闭开关会取消并清理私有查询。支持 `zh-CN` / `en-US`；没有桌面后端的网页预览只展示真实能力限制。

最新发布 tag 为 **v0.1.0-beta.3**；main 的后续功能不等于该 tag 已发布。安装包、许可材料、平台验收与待验证项见 [发布矩阵](docs/release-matrix.md)，备份、恢复和手动升级见 [维护说明](docs/maintenance.md)。当前本地/浏览器检查不能替代各平台安装、系统关联打开、keyring 或实时 Provider 验收。

## 开发环境

- Node.js 24 LTS（前端最低 22.12）。
- **pnpm 12.17.0**：固定在 `package.json`；CI 读取同一版本。升级后同步锁文件。
- Rust **1.90.0**，由 `rust-toolchain.toml` 固定，含 rustfmt 和 clippy。
- [Tauri 2 平台前置依赖](https://v2.tauri.app/start/prerequisites/)：Windows 需要 MSVC C++ Build Tools 和 WebView2；macOS 需要 Xcode Command Line Tools。
- Ubuntu/Debian 还需 GTK/WebKit 开发包，例如：

```bash
sudo apt-get update
sudo apt-get install -y build-essential libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev libdbus-1-dev librsvg2-dev patchelf
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

桌面启动后添加媒体目录，通过系统文件夹选择器选择目录及类型。目录自动在后台扫描、建立索引并补全可靠匹配的资料，本地媒体先显示，无需手动“入库”；只需处理未识别或失败项目。媒体仍由系统关联应用打开，源文件保持只读。“设置 → 诊断与维护”提供数据库备份/恢复与缓存维护；网页模式无法调用后端，不伪造媒体记录。完整行为见 [自动媒体库](docs/automatic-library.md)。

默认使用本机应用数据目录中的 `library.sqlite`；扫描阶段只读；视频输出仅在用户明确确认后创建 NFO/海报 sidecar，已有文件保持原样。P6 音频标签修改和恢复各自要求 Preview 与明确确认，备份原字节保留，外部改动或来源关联变化时拒绝旧计划。海报使用本地缓存缩略图，远程图片由受控后端获取。成人资源使用统一前端及独立后台授权，关闭开关会清除相关查询缓存。语言选择持久化，默认 `zh-CN`，缺失翻译回退 `en-US`。

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
- `.github/workflows/release.yml`：只由 beta tag push 触发，Windows x64、Linux x64、macOS x64/ARM64 四个原生 runner 并行构建。验证安装器实际 payload 架构/版本/许可后，汇总 SHA-256 与 manifest，完整上传 draft 后才公开 beta。Windows/Linux ARM 本次明确不构建。普通分支 push/PR 不发布。
- `.github/tauri.release.conf.json` 只在 tag 构建时启用打包；本地 `pnpm build:desktop` 仍只生成可执行文件。
- main 的 Windows 便携 ZIP 只包含 `MediaAtlas.exe`，运行依赖系统已安装的 WebView2。第三方许可全文嵌入 EXE，可用 `MediaAtlas.exe --export-licenses <新目录>` 导出；Release 仍单独提供 `license-materials.zip`。此包装调整未纳入既有 beta.3 tag。
- tag 名称不会自动修改应用版本，发布前需同步 Cargo workspace、Tauri 和前端包的版本。本 beta 未接入发行签名、Apple 公证或自动更新，请按手动更新说明操作；CI 包验证不替代交互式安装/卸载、实机播放、OS keyring 或实时 Provider 验收。

工作流配置及静态检查不等于远端 CI、可下载安装包或平台交互已验证，实际状态以发布矩阵为准。

## 本地存储示例

```bash
cargo run -p mediaatlas-infrastructure --example library_roundtrip --locked
```

该示例只在临时目录创建 Fixture SQLite，验证 Root/媒体/文件索引、按 Root 分页、人工字段保护及关闭重开；不会扫描、移动或写入真实媒体。桌面启动打开应用自身 SQLite；只处理用户明确添加、启用的媒体目录。

文件组织支持 Preserve / Move / Copy / Hardlink、关联附件、执行日志及独立回滚预览；保留原始恢复字节，中断后不自动重放文件修改。目录监听按用户显式开启的范围工作。音乐保留独立图谱、标签写入和恢复边界。

## 目录与契约

```text
apps/desktop/src/             React、路由、i18n、typed IPC
apps/desktop/src-tauri/       Tauri 宿主、窗口、command/event、最小权限
crates/domain/               公共实体、独立媒体模型、路径与元数据验证
crates/application/          应用用例、端口与版本化传输模型
crates/desktop-bridge/       不依赖 Tauri 的 DTO 和错误协议
crates/infrastructure/       SQLite、网络、扫描/输出服务及追加迁移
crates/media-scanner/        只读遍历、分类型文件名解析、本地 NFO/音频标签
docs/                        当前流程、架构决策、接口契约、维护与发布
```

P0 `get_app_info` 契约不变；P2 新增 Root/浏览/扫描命令，返回 `{schema_version:1,data:T}`。任务事件用于刷新提示，持久化查询和轮询为权威来源，UI 不依赖事件必达。完整契约见 [Phase 2 接口](docs/phase-2-contract.md)。前端不访问刮削站点。

日志写入 Tauri `app_log_dir`，每日滚动、最多保留 7 份，并向 stderr 输出 JSON。P0 日志只记录固定启动信息，不接收用户 payload；网络和文件业务的脱敏在对应阶段落实。

当前文档见 [docs/README.md](docs/README.md)，视觉与开发规范见根 [design.md](design.md)和两级 `AGENTS.md`。旧实施计划、阶段成绩和旧 UI 走查已移除，不作为当前行为或本轮验收依据。

当前 P0 已实现 [持久化人工核对队列](docs/review-queue.md)及 schema 19：最近队列、逐项状态与确认检查点以实际后端账本为准。本轮前端 397 项、Rust 343 项测试及构建检查通过，见[队列截图](screenshots/review-queue-2026-11-04/README.md)。P1 来源顺序、资料语言与补图策略仍为后续契约。

## 验证与截图

本轮验证重新运行适用的前端、Rust、IPC 和发布检查。实际浏览器截图、源码和数据模式见 [screenshots/README.md](screenshots/README.md)。浏览器合成 IPC 验证界面与交互，不证明真实系统目录窗口、keyring、在线来源或媒体写入已通过。

## 许可与分发

项目源码许可状态见 [LICENSE-STATUS.md](LICENSE-STATUS.md)。第三方许可说明、完整清单与原文/必要源包分别保存在 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)、[dependency-licenses.json](docs/dependency-licenses.json)及 `docs/third-party-licenses/`。这些材料是构建和发布输入，必须随分发包保留，不能作为旧文档或缓存清理。
