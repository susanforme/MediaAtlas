# Phase 3 实施与验收记录

- 日期：2026-11-03（Asia/Shanghai）。
- 状态：P3实现、核心和前端本地验收通过；原生GUI启动有已确认环境限制，跨平台待验收事实保留。
- 基线：先拉取最新 main 至 `1320fb3`，包含 P2 扫描、媒体库和任务契约；用户最新指示覆盖早期开发分支要求，P3 在 main 集成。
- 分工：主集成先冻结 `provider-contract.md` 和共享domain契约；network子代理实现HTTP/API/HTML和策略，runtime子代理实现注册/持久化/任务调度，UI子代理实现Provider工作台和IPC校验，网络测试子代理验证Mock契约；主集成实现密钥适配/宿主接线并修复审查问题。
- 提交身份：Codex <codex@openai.com>。本P3实现单元仅覆盖P3；按用户后续连续授权，验收提交推送后由主代理安排下一阶段，不创建tag/Release。

## 完成范围

| 任务 | 实现 |
| --- | --- |
| P3-01 | reqwest共享连接池、JSON/GraphQL和有界分页、TLS native roots、显式代理安全边界、凭据引用解析 |
| P3-02 | HTTP获取HTML，CSS/JSON-LD/Open Graph/embedded JSON静态解析；不执行页面脚本、不用浏览器刮削 |
| P3-03 | 完整Descriptor、分能力trait、可扩展后端adapter registry、不可变Job与配置revision；默认只有禁用的离线Fixture |
| P3-04 | 全局/单站/图片并发、站点节流、GET有界重试/Retry-After、健康冷却与恢复探测、全链路取消 |
| P3-05 | HTTPS/公网目标校验、DNS结果用于实际connector、重定向重新校验且跨origin不传密钥、流式响应上限、脱敏摘要、系统凭据和显式会话降级 |
| P3-06 | Providers设置、Task Center保留P2扫描任务并增加Provider任务、Network Inspector；持久化配置/任务/运行/脱敏记录，重启只恢复Provider非终态为Interrupted |
| P3-07 | Mock HTTP、离线组合Fixture、任务持久化与前端IPC/交互测试；不依赖实时站点 |

## 可运行流程

`pnpm dev` 启动桌面，在 Providers 中显式启用 Offline Fixture、输入检索文本并运行；Task Center 可取消和重查历史；Network Inspector 显示真实执行记录。Fixture仅组合本地JSON与HTML：`method=FIXTURE`、`status=null`，不会伪装为远端HTTP成功，也不会写入媒体库。开启Provider不会自动启动工作。

`provider_snapshot` 等命令遵循P2 `ApiResponse<T>` envelope；UI验证字段/版本/脱敏URL，网页模式显示需要桌面。语言沿用P2 `zh-CN/en-US`，Job显式固化语言和独立对象上下文，不读取UI当前选择。

## 实际验证

| 检查 | 实际结果 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、Vitest、生产构建通过；42 项前端测试（含20项P3定向契约/交互测试） |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 全 workspace 含宿主和所有测试通过 |
| `cargo test --workspace --locked` | 全 workspace 89 项测试通过，无失败；不将0项doc-test当功能测试 |
| Chromium真实浏览器 | Providers、Network Inspector、Task Center 三路由在 zh-CN/en-US 渲染正常；无pageerror；语言切换刷新恢复通过。浏览器模式仍正确显示需要桌面 |
| Linux原生桌面 | 编译及链接通过；启动受rootless sysroot WebKit helper硬编码路径限制失败，未验收GUI/原生IPC端到端 |

P3新增验证包括20项真实Mock HTTP/TLS/DNS网络测试、3项双Mock adapter运行时测试、7项持久化/生命周期集成测试、2项会话凭据防泄露测试；日志测试新增第三方URL/header诊断及应用debug过滤负例。测试验证冷却跨站隔离、晚到成功不覆盖429、恢复探测取消、排队/重试不占健康站全局容量、实际dispatch间隔、重定向密钥移除、chunked cap、分页/GraphQL组合、配置/语言快照、重启中断和成人adapter拒绝。

审查修复：reqwest内部重试显式关闭；日志仅允许本应用INFO以上固定事件；DNS用于实际connector且所有重定向重验；按task有界请求缓冲完成即清理，截断有明确标记；原生密钥任务用全局2个permit，超时原生调用仍占permit，防止后台线程失控。

## 安全边界与平台限制

- 无实时站点适配器、无媒体写入/刮削/合并；这些在后续P4实现单元完成。普通Movie执行通道拒绝AdultVideo adapter，P2成人媒体隔离不被Provider工作台绕开；未来其他类型必须新增显式上下文/隔离策略。
- Linux/Windows/macOS系统密钥分别使用Secret Service/Credential Manager/Keychain后端。当前无可用系统密钥服务，实际OS读写未验收；返回稳定不可用错误，可显式选择不落盘SessionOnly，不自动保存明文。P3 UI仅接收reference，不提供secret输入/回显。
- OS密钥调用放到后台并设5秒请求期限；原生调用不可强制取消，超时后后台操作可能稍后完成。会话密钥不会持久化，进程退出丢失。
- 带环境代理时不能证明代理DNS目标的应用默认拒绝联网；可信gateway仅后端代码显式配置，必须保证同等私网/metadata限制，不提供IPC信任开关。当前Fixture无外网访问。
- Linux原生启动所用解包sysroot无法满足WebKit发布库硬编码 `/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitNetworkProcess` helper路径，当前实例没有root安装权限。未改源码绕过、未禁用沙箱，原生GUI/IPC未验收。
- Linux当前实例验证不代表Windows/macOS启动、跨平台远端CI、发布包或实时Provider授权通过。浏览器检查不等于原生IPC端到端验收。
