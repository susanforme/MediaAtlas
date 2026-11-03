# Phase 3 Provider contract (v1)

P2 已合并到 main（1320fb3），P3 基于 P2 任务/IPC/数据库和 UI 开发；保留 P2 平台待验收事实。按用户最新指示直接在 main 开发，所有提交使用 Codex 身份。

## 冻结边界

- `domain::provider` 定义分能力 Search/Metadata/Artwork trait、ProviderDescriptor、ProviderConfig、ProviderJob、ProviderFailure、NetworkRequestRecord。Job 持有对象、query、config 的不可变值；任务只返回结果，不写入“当前选中媒体”。
- `infrastructure::network` 提供共享 `NetworkEngine`、`CancellationToken`、`RequestSpec`、`EngineResponse`；统一 REST/GraphQL JSON 与 HTTP/HTML。HTML 提取 CSS、JSON-LD、Open Graph、内嵌 JSON，不执行网页脚本/浏览器。运行时凭据通过 CredentialStore 注入，持久化只存引用；无系统密钥服务时明确报不可用并允许显式会话内降级，绝不明文落盘。
- NetworkEngine 接收显式 config 快照；通过全局/站点/图片 semaphore、节流、Retry-After、健康冷却和取消控制请求。配置更新只影响新 Job。所有跳转重新校验 HTTPS、hostname/IP/DNS；直连 DNS 固定解析结果避免重绑定；生产禁止私网/metadata，测试专用 loopback policy 不可经 IPC 启用。限制流式响应体积和重定向/重试次数。不打印原始错误/请求体/查询/header。
- `infrastructure::providers` 持有注册配置与 task/网络记录 SQLite 持久化，重启将非终态标 Interrupted。只注册明确标记的离线 Fixture Provider：组合 JSON/HTML fixture 验证生命周期，不产生外网访问；后续 P4 才注册真实站点。
- v1 IPC 沿用 P2 `ApiResponse<T>` 版本 envelope、`{input}` 参数规则: `provider_snapshot() -> ApiResponse<ProviderSnapshot>`、`provider_set_enabled({provider_id, enabled}) -> null`、`provider_update_config({provider_id, config}) -> null`、`provider_start({provider_id, query, language}) -> String`、`provider_cancel({task_id}) -> bool`。快照字段 `schema_version:1, providers:ProviderView[], tasks:ProviderTaskView[], requests:NetworkRequestRecord[]`。UI 分 Providers/Task Center/Network Inspector，浏览器显示需要桌面，不制造成功数据；zh-CN/en-US、持久化语言选择。
- DTO 结构由 domain::provider 导出 serde，与 frontend TS严格对应。ProviderView `{descriptor, config, health}`；ProviderTaskView `{id, provider_id, state, query, created_at, updated_at, failure, result_summary}`。时间 ISO UTC，state 使用既有 TaskState；失败稳定分类本地化。
- 最大历史列表100条，所有显示URL仅 origin（无用户名、path、query、fragment）；网络记录只包括 task/provider ID、engine、method、safe_url、duration_ms、status、failure、attempt。查询输入不持久化凭据；Fixture 输入限定媒体检索文本，不接受URL/headers。

## 验收

常规 CI 使用 Mock HTTP 和 Fixture：JSON/HTML组合、分页/GraphQL、DNS/TLS/timeout/network/HTTP/阻断/验证码/限流/解析/身份错误；响应上限、SSRF/跳转、取消等待与请求、冷却恢复、配置隔离、并发限制、数据库重开、启停与中断恢复、UI实际IPC映射。无实时站点成功率/跨平台验收声明。

## 集成约束补充

- 描述元数据完整包含 documentation/terms/privacy/website、engine_types、requires_api_key/login、rate_limit_policy、attribution、data_usage_notes；不声明未接入站点的支持。
- Job 固化 media_kind、media_item_id、file_identity、language、region、correlation_id 和完整 config。P3自测不写用户媒体对象；P4对象绑定必须显式携带，不能使用UI当前选择。
- 后端 registry 支持注册多个适配器，适配器使用同一 NetworkEngine 和取消上下文；IPC不提供任意URL/任意适配器注册入口。离线 Fixture 的记录 method=FIXTURE、status=null，不能被描述为HTTP成功。
- keyring 适配器接系统密钥服务（Linux Secret Service / Windows Credential Manager / macOS Keychain），绝不把secret放argv/log/数据库。后台阻塞任务隔离系统调用、请求5秒超时，不可用返回 `credentials_unavailable`，不会自动明文保存。系统原生API不支持强制取消，超时后后台操作可能仍在完成，UI不会无限等待。`SessionOnly` 必须由调用者显式选择，清空/进程退出即丢失。Windows/macOS系统密钥后端已选对应原生实现，但当前Linux无密钥服务，真实OS凭据读写及其他平台仍未验证。P3 UI只保存reference、不接收或回显secret；实际站点授权交互留待P4按Provider访问条件接线。
- 使用HTTPS代理时，应用DNS检查不证明代理的解析结果。默认拒绝未受信代理，可信gateway只能由宿主代码构造并须负责同等目标限制，不能由IPC开关绕过。当前环境有代理但Fixture不出网；常规测试使用限test构造的loopback policy。

- 配置边界：UI逐站可调启停、timeout、并发/图片并发、最小请求间隔、响应上限、重试与credential_ref；revision由后端分配。全局/全局图片并发和可信代理在NetworkEngine后端构造，User-Agent默认应用标识，可信适配器可经RequestSpec设置有界User-Agent和已校验Referer。逐站代理/UA/Referer的任意字符串UI配置未实现；后续站点transport profile由可信后端注册，不将任意代理或身份header交给UI。
- P3公开start_context实际支持普通Movie、language、region与correlation快照。media_item_id/file_identity字段预留但本阶段显式拒绝非空；不伪称已接入P4对象匹配与回写。

- 请求记录按task有界收集并在任务完成时take/清理；单任务超过128条显式标记 `request_history_truncated`。SQLite列表只返回最近100条，不宣称界面展示所有历史。
