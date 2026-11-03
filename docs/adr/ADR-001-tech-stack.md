# ADR-001：桌面技术栈与初始工程边界

- 状态：Phase 0 采用。
- 日期：2026-11-03。
- 基线：第 3、4、13 节。

## 背景

MediaAtlas 是 Windows 优先、兼容 macOS/Linux 的本地媒体库管理应用。桌面能力、业务核心和 UI 需要独立测试，网络采集不能散落在前端。当前仓库从文档开始，初始化时应保持工程规模与实际功能相符。

## 决策

- 桌面使用 Tauri 2；核心 Rust stable；前端 React + TypeScript + Vite；包管理 pnpm + Cargo。
- UI 按基线采用 Tailwind CSS、shadcn/ui 和 lucide-react；实际组件随页面需要引入，不预生成整套组件。TanStack Router、TanStack Query、少量 Zustand 分别承担路由、服务端状态和局部跨组件状态；没有实际状态需求时不创建空 store。
- Tauri Commands 承担请求/响应，Events 承担通知；前端通过 typed IPC 消费版本化 DTO。P0 诊断协议见 [architecture.md](../architecture.md)。
- P0 使用 `domain`、`application`、`infrastructure`、`desktop-bridge` 四个最小 core crate；桌面宿主放在 `apps/desktop/src-tauri`。bridge 负责纯 DTO 转换，Tauri 依赖只在宿主中。
- `scraper`、`media-scanner`、`media-export` 在对应阶段确有代码和边界需求时建立。Provider 实现逐个接入，不预建候选站点空模块。
- 工程检查使用 rustfmt、clippy、cargo test、Biome、Vitest；Playwright 仅可用于后续 UI E2E，不进入刮削运行时。
- 工具链与依赖通过版本声明和锁文件固定；Windows 是第一验收平台，macOS/Linux 持续编译。工作流只产生验证/构建产物，不自动发布版本。

## 影响与基线落地说明

采用基线技术栈，不引入 Electron 或 Node 后端。基线 Phase 0 列出的完整模块目录采取分期落地：P0 不创建尚无职责的 scraper/scanner/export 空 crate。原因是基线第 4.2 节要求少量 crate、避免空抽象；影响是后续阶段需要追加 workspace 成员，替代方案是先建立全部空 crate，但会增加无行为代码与维护开销。

前端开发预览不能证明桌面 IPC 可用；所有平台构建与启动结果必须有证据。当前可用版本和缺失工具以 [repo-audit.md](../repo-audit.md) 及阶段验收记录为准。
