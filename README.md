# MediaAtlas

Windows 优先、兼容 macOS/Linux 的本地媒体管理桌面应用。采用 Tauri 2、React、TypeScript 和 Rust。

当前仅实现 **Phase 0 工程骨架**：空桌面窗口、简体中文/英文界面、本地核心连接检查、真实构建版本、版本化 IPC、结构化滚动日志和 CI。尚无扫描、数据库或 Provider 功能。浏览器预览会明确提示需要桌面运行。

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

桌面启动后应显示“本地核心已连接”和构建版本 `0.1.0`。切换语言、重新检查连接属于 P0 手动验收；网页模式应显示“请在桌面应用中打开”。

## 检查

```bash
pnpm check                             # Biome、TypeScript、Vitest、前端构建
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

默认 Rust 成员为四个 core crate，可在未安装 GTK/WebKit 的环境独立测试。完成前端构建、安装平台依赖后，检查完整桌面 workspace：

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

`.github/workflows/ci.yml` 配置 Windows/macOS/Linux 原生检查，Windows 额外构建可执行文件并执行进程启动冒烟检查。该检查仅验证进程持续运行，窗口渲染和 IPC 仍需手动验收；配置工作流不等于已通过远端 CI。不创建 GitHub Release 或自动更新发布。

## 目录与契约

```text
apps/desktop/src/             React、路由、i18n、typed IPC
apps/desktop/src-tauri/       Tauri 宿主、窗口、command/event、最小权限
crates/domain/               领域错误；后续独立媒体模型
crates/application/          应用信息用例
crates/desktop-bridge/       不依赖 Tauri 的 DTO 和错误协议
crates/infrastructure/       结构化日志适配
docs/                        基线、架构决策、任务与验收
```

当前仅 `get_app_info` 命令，`schema_version: 1`。启动事件 `app-ready` 仅为诊断，不保证 UI 必达；UI 通过命令握手。前端不访问刮削站点，不把浏览器预览伪装成核心连接成功。

日志写入 Tauri `app_log_dir`，每日滚动、最多保留 7 份，并向 stderr 输出 JSON。P0 日志只记录固定启动信息，不接收用户 payload；网络和文件业务的脱敏在对应阶段落实。

- [原始实施基线](docs/UNIVERSAL_MEDIA_SCRAPER_AGENT_PLAN.md)
- [阶段任务](docs/agent-plan.md)
- [架构与 IPC](docs/architecture.md)
- [初始仓库审计](docs/repo-audit.md)
- [Phase 0 验收记录](docs/acceptance/phase-0.md)

本轮按要求在 Phase 0 停止，不启动 Phase 1。
