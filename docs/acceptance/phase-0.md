# Phase 0 实施与验收记录

- 日期：2026-11-03（Asia/Shanghai）。
- 状态：**工程实现完成、本地自动化检查通过，阶段验收待完成**。
- 范围：只实施 P0；按用户要求停止，不进入 P1；不发布应用。
- 提交：以本记录随附的 `chore: bootstrap tauri workspace` 提交为准。

## 分工与产物

| 任务 | 实现结果与路径 | 验证状态 |
| --- | --- | --- |
| P0-01 工具审计 | `docs/repo-audit.md`、固定 Rust 1.90.0/Node 24/pnpm 12.17.0 | 当前 Linux 工具版本已验证；Windows/macOS 工具环境待 CI 验证 |
| P0-02 架构与 ADR | `docs/architecture.md`、`docs/adr/ADR-001` 至 `ADR-004` | subagent 独立审查；接口和边界已冻结 |
| P0-03 workspace 与桌面骨架 | `apps/desktop/`、`crates/`、Cargo/pnpm manifests 和锁文件 | 前端生产构建、Linux 原生编译及链接通过 |
| P0-04 错误、日志、DTO | `crates/domain`、`application`、`desktop-bridge`、`infrastructure`、`apps/desktop/src-tauri` | 契约/错误安全文本/结构化日志测试通过 |
| P0-05 工程检查和 CI | `biome.json`、`rust-toolchain.toml`、`.github/workflows/ci.yml`、`scripts/smoke-windows.ps1` | 本地检查通过；远端 CI 结果未确认，Windows 启动待验证 |
| P0-06 开发入口与记录 | `README.md`、本文件 | 启动/构建/测试步骤已文档化；不标记阶段全部验收通过 |

subagent 分别负责架构、前端、Rust/Tauri；主代理负责工具链、CI、集成、测试及记录。架构审查发现 Tauri 图标即使不打包也为必需，现已补齐原创 SVG 源及 PNG/ICO；没有复制参考项目代码或资产。

## 可运行产物与行为

- `apps/desktop/dist/`：前端生产构建；未跟踪构建产物。
- `target/debug/mediaatlas-desktop`：通过 Tauri CLI 生成的 Linux 调试可执行文件；未跟踪构建产物。
- 使用 `pnpm dev` 启动桌面开发；`pnpm dev:web` 仅作网页预览。
- 界面包含简体中文/英文、本地核心连接状态、真实版本和重试。浏览器预览明确提示缺少桌面环境，不伪造连接成功。
- 唯一 IPC command 为 `get_app_info`，协议版本为 1；启动时发送 `app-ready` 诊断事件，UI 不依赖事件必达。
- Rust 核心不依赖 Tauri；不含数据库、扫描、Provider 或媒体业务实现。
- 结构化日志只接受固定启动诊断，滚动保留；系统密钥存储和网络脱敏尚未接入，不能据此声明已验证后续能力。

## 实际执行的检查

| 命令 | 结果 |
| --- | --- |
| `pnpm --version`（项目指定工具） | `12.17.0` |
| `pnpm install --frozen-lockfile` | 成功；使用临时 store/cache，锁文件与 manifests 一致 |
| `pnpm check` | Biome、TypeScript、Vitest 和 Vite 构建全部成功；2 个测试文件，11 项测试通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --all-targets --locked -- -D warnings` | 通过，默认四个 core crate |
| `cargo test --locked` | 5 项核心测试通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 包含 Tauri 宿主的完整 workspace 通过 |
| `cargo test --workspace --locked` | 包含 Tauri 宿主的编译/链接及测试通过；5 项核心测试，宿主尚无独立测试用例 |
| `pnpm --filter @mediaatlas/desktop tauri build --debug --no-bundle` | 成功生成 Linux 原生可执行文件 |

当前沙箱默认用户目录不可写，因此实际 pnpm 检查使用临时 `XDG_DATA_HOME`、`XDG_CACHE_HOME`、store 和 pnpm 12 二进制路径；Rust 使用临时 `RUSTUP_HOME` / `CARGO_HOME`。这些是执行环境适配，不改变仓库或用户的 HOME。

Linux 完整 workspace 检查使用临时解包的 Debian GTK/WebKit 开发库，设置 `PKG_CONFIG_PATH`、`PKG_CONFIG_SYSROOT_DIR` 和 `LD_LIBRARY_PATH`。常规开发机和 CI 按 README 安装平台依赖，不需要该临时 sysroot。

## 平台启动与 CI 的未完成项

| 项目 | 实际状态与剩余验证 |
| --- | --- |
| Linux 虚拟显示启动 | 尝试通过 Xvfb 启动原生应用；WebKit 运行库从预设 `/usr/lib/.../WebKitNetworkProcess` 加载子进程，临时 sysroot 中虽有该程序但系统路径不存在，应用退出。**未通过 GUI/IPC 启动验收**，编译成功不等于运行验收通过 |
| Windows | 已配置原生 CI、可执行文件构建、10 秒进程存活检查；尚未获得实际运行结果。该检查本身也不证明窗口渲染和 IPC 成功，仍需 Windows 桌面手动验证 |
| macOS | 已配置原生 CI 编译/检查，尚未获得执行结果 |
| GitHub Actions | 工作流已创建，远端运行结果尚未确认；当前通过 `gh api` 查询 Actions 被环境返回 Forbidden，无法读取运行结果。不能用工作流配置代替 CI 成功证据 |

阶段通过仍需：Windows 空桌面应用可启动且显示真实核心连接与版本，Windows/macOS/Linux 的 CI 检查成功。所有未验证结果保留为待验收，不伪造通过记录。

## 设计偏差及理由

- 基线 P0 推荐七个 core 模块；本次只建立四个有实际职责的 core crate。原因是基线同时要求少量 crate、避免空壳。scanner/scraper/export 随对应阶段引入，边界原则不变，详见 ADR-001。
- `desktop-bridge` 保持纯 DTO 转换，Tauri command/event 适配位于 `apps/desktop/src-tauri`，保证核心测试不依赖桌面库。
- Biome 排除 Markdown 和生成目录，避免自动格式化原始基线；代码和配置仍执行完整检查。
- 按用户最新指示固定 pnpm 12.17.0，替换初始环境中的 pnpm 11；没有更改技术栈或产品范围。

## 停止点

本轮提交并推送 Phase 0 后停止，等待用户指示。未开始 Domain RFC、SQLite Schema、迁移或任何 Phase 1 实现。后续若恢复，先补齐上述平台验收；未满足阶段门槛前不进入 P1。每阶段都必须先提交，再等待用户明确要求开始下一阶段，不能自动推进。
