# ADR-001：桌面技术栈与初始工程边界

- 状态：Phase 0 采用。
- 日期：2026-11-03。
- 当前相关文档：[架构与 IPC](../architecture.md)、[自动媒体库](../automatic-library.md)。

## 背景

MediaAtlas 是 Windows 优先、兼容 macOS/Linux 的本地媒体库管理应用。桌面能力、业务核心和 UI 需要独立测试，网络采集不能散落在前端。工程规模与实际功能保持一致，不引入没有职责的空模块。

## 决策

- 桌面使用 Tauri 2；核心 Rust stable；前端 React + TypeScript + Vite；包管理 pnpm + Cargo。
- UI 按基线采用 Tailwind CSS、shadcn/ui 和 lucide-react；实际组件随页面需要引入，不预生成整套组件。TanStack Router、TanStack Query、按需的局部状态分别承担路由、服务端状态和界面交互；没有实际状态需求时不创建空 store。
- Tauri Commands 承担请求/响应，Events 承担通知；前端通过 typed IPC 消费版本化 DTO。P0 诊断协议见 [architecture.md](../architecture.md)。
- P0 使用 `domain`、`application`、`infrastructure`、`desktop-bridge` 四个最小 core crate；桌面宿主放在 `apps/desktop/src-tauri`。bridge 负责纯 DTO 转换，Tauri 依赖只在宿主中。
- `scraper`、`media-scanner`、`media-export` 在对应阶段确有代码和边界需求时建立。Provider 实现逐个接入，不预建候选站点空模块。
- 工程检查使用 rustfmt、clippy、cargo test、Biome、Vitest；Playwright 仅可用于后续 UI E2E，不进入刮削运行时。
- 工具链与依赖通过版本声明和锁文件固定；Windows 是第一验收平台，macOS/Linux 持续编译。main/PR 工作流执行检查；只有明确的 beta tag 发布流程才能分发版本，具体状态见发布矩阵。

## 影响与基线落地说明

采用上述技术栈，不引入 Electron 或 Node 后端。按实际依赖与复用边界增加模块，不为计划目录创建空 crate。

前端开发预览不能证明桌面 IPC 可用；所有平台构建与启动结果必须有证据。当前可用版本和缺失工具以 [发布矩阵](../release-matrix.md) 及[当前截图记录](../../screenshots/README.md)为准。
