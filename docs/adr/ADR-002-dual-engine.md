# ADR-002：API 与 HTTP/HTML 双引擎

- 状态：架构采用；P3 实现并验收。
- 日期：2026-11-03。
- 基线：第 3.1、3.2、6 节。

## 背景

媒体来源同时存在结构化 API 和普通 HTML。Provider 是站点业务适配，不等同于传输或解析引擎。将每个站点做成独立 HTTP 栈会导致超时、限流、日志和取消行为不一致。

## 决策

- Rust 后端统一采集。API Engine 负责 REST/GraphQL/JSON、分页及鉴权；HTML Engine 负责普通 HTTP、CSS Selector、JSON-LD、Open Graph 和内嵌 JSON。
- 两种引擎共享受控 reqwest Client、连接池、超时、代理及请求策略；Provider 可组合两种引擎。
- Provider 按搜索、详情、图片等能力分接口，公开稳定标识、类型/语言支持、访问条件、条款和许可；不要求所有 Provider 实现万能 Trait。
- 每个 Job 固化 query、文件身份、媒体类型、Provider 配置、语言/地区和关联 ID，支持取消；不读取 UI 的“当前选中媒体”。
- 各 Provider 独立限流、并发、Retry-After 冷却和健康状态；区分网络、HTTP、限流、阻断、验证码、未找到、解析失败、身份不符等失败。
- SSRF 防护覆盖 URL、地址解析和重定向目标；限制响应大小、下载类型和图片解码资源。凭据走系统密钥存储，日志脱敏。具体实现与测试列入 P3，不在 P0 声称完成。
- 不使用 Playwright、Puppeteer、Selenium 或内置 Chromium 采集。JS 站点评估公开 JSON 接口；无法稳定 HTTP 获取则标记不支持或部分支持。
- 不绕过登录、验证码、付费墙或访问控制；403/429/验证码页作为明确状态处理。

## 影响

CI 使用 Mock HTTP 和本地 Fixtures，实时连通性只作为手动/可选诊断。站点可用性与接入条件决定支持矩阵，候选站点列表不构成承诺。P0 不添加网络访问或演示抓取；P3 先冻结 Provider 契约再实现框架。
