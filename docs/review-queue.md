# 持久化人工核对队列

当前持久队列与实时合并的实现契约。默认目录流程仍是后台自动发现和可靠匹配；队列用于普通影视的手动核对，不增加入库门槛。产品主线见 [自动媒体库](automatic-library.md)及 [Emby 流程基准](emby-scraper-workflow.md)。

## 范围和入口

- 从普通媒体工具的真实服务端清单选择 1–32 个 Movie / TV Series / Anime ID，提交前保留顺序并去重。后端再次核验本地文件、目录、类型和成人隔离，不因前端选择就授予权限。
- 创建后使用 `/scraper?session=<queue UUID>&media=<media UUID>` 进入当前项。最近队列入口读取后端账本；`list` 最多返回最近 20 队列，不宣称无限历史分页。
- 返回清单只使用受限 `ReviewContext`：`root`、`kind`、`health`、`q`、`sort`、`cursor`、`page`。返回地址固定在应用内部；上下文不提供任意返回 URL、执行参数、目录路径或凭据字段。检索词不是秘密存储入口，服务端校验长度、控制字符、URL/凭据形态，以及游标的工具查询范围。
- 成人和音乐不进入此队列。成人保留独立授权、缓存和现有工具流程，不能借普通账本恢复私有资源。

## 状态与可验证的进展

| 状态 | 含义 |
| --- | --- |
| `pending` | 需要核对，尚未有此队列可证明的确认结果 |
| `deferred` | 用户暂缓，可稍后显式继续 |
| `failed` | 当前处理未完成，可显式重试 |
| `confirmed` | 绑定的后端合并预览已经成功确认，不能由页面位置或客户端计数伪造 |

上一项、下一项、暂缓和恢复只是选择/账本操作；它们不搜索、不自动选候选、不确认资料，也不写旁车。失败/暂缓的 `retry` 重新核验当前资源后回到 `pending`；它不是自动重放网络或文件修改。

读取队列重新检查资源与目录范围。已删除、改为成人、目录停用或关联改变的项保留记录以供说明，不静默删除或当作成功。返回的 `needs_recheck` / `reason` 区分 `missing_media`、`unavailable_media`、`expired_item` 和 `stale_checkpoint`。当前项位置与确认数量是不同事实。

## 实时合并界面

选定候选后直接进入双栏工作区。左侧以真实本地资料为基础，展示当前补充来源、字段与“仅补空缺”选项；右侧在每次选择时立即更新合并后画面、前后对比与逐字段明细；底部汇总新增、覆盖、图片及跳过数量。状态使用非交互 shadcn Badge，对比模式使用支持方向键的共享 Tabs。图片只使用已有缓存或既有安全 IPC 返回的缩略图；选中图片会显式请求对应缩略图，单独预览图片不会自动选入合并。

前端投影不构成确认凭据。选择稳定 200ms 后自动请求对应 `scraper_preview`，等待、失败、零变更或旧响应期间禁止确认；失败允许显式重试，不循环自动请求。确认前再次核对后端实际选择与当前画面的目标、字段、语言、值和图片集合完全一致。其他任务刚填入字段、导致“仅补空缺”与后端预览不一致时，必须刷新本地资料和预览，避免将界面显示为跳过的字段提交覆盖。最终后端事务校验、锁定与手改保护继续生效。

无需再单独点击“预览合并”；检索、选择候选、确认合并和旁车输出仍为不同动作，实时预览不自动确认或写旁车。队列导航保留在工作区外，切项、暂缓、离开继续忽略晚到结果并回收输出预览。

桌面壳固定在窗口高度，只有右侧内容区域滚动；侧栏导航项过多时内部滚动，底部设置入口保持可见。跨路由回到右侧顶部，字段编辑保留滚动位置。

## IPC

全部沿用 `invoke(command, {input})`、`{schema_version:1,data:T}` 及稳定安全错误；ID 为 UUID 字符串。精确字段以 [Rust DTO](../crates/application/src/review_queue.rs)和 [TypeScript API](../apps/desktop/src/api/review-queue.ts)为准。

| 命令 | input | data |
| --- | --- | --- |
| `review_queue_create` | `media_ids`, `context` | `ReviewQueueView` |
| `review_queue_list` | `{}` | 至多 20 个 `ReviewQueueView` |
| `review_queue_get` | `queue_id` | `ReviewQueueView` |
| `review_queue_update` | `queue_id`, `media_id`, `expected_revision`, `action`, 可选 `checkpoint` | 更新后的 `ReviewQueueView` |
| `review_queue_checkpoint` | `queue_id`, `media_id`, `expected_revision`, `preview_id` | 绑定预览后的 `ReviewQueueView` |

`ReviewQueueView` 包括 `id`、`revision`、`current_media_id`、`context`、有序 `items`、创建/更新时间。每项包含媒体 ID、位置、状态、预览/检查点 ID 和重核原因。更新 action 为 `select | defer | retry | failed | confirmed`；没有通用“客户端直接指定任意状态”接口。

## 并发、确认与恢复

- 创建和更新在 SQLite 写事务内核验目录/媒体范围；更新携带 `expected_revision`，旧页面或并发修改返回冲突，不能覆盖新账本。
- `checkpoint` 只能绑定当前 `pending` 项的真实、近期、`planned` 普通合并预览，媒体身份和队列创建时间必须匹配。服务器生成检查点 UUID，不接受 UI 自造确认凭据。
- 现有 `scraper_confirm` 的最终写事务再次验证该预览绑定的队列 revision、当前项、状态、检查点及资源范围。切换、暂缓、失败或新检查点使旧绑定失效；旧绑定记录保留，不能通过清空当前项绕过确认保护。
- `confirmed` 必须由成功的、同媒体绑定预览证明。资料已经提交但账本响应丢失时，重新读取可以根据后端成功记录推导已确认；读取本身不重放合并或文件写入。
- 检查点窗口为 30 分钟，待核对项检查窗口为 7 天；过期或资源范围变化需要显式重新核验，并生成新的预览。记录不因到期自动删除，也不自动执行旧预览。
- NFO/海报、标签和文件组织仍使用自己的独立预览/确认。核对账本不构成任何磁盘写入授权。

实现见 [ReviewQueueService](../crates/infrastructure/src/review_queue.rs)与[前端状态边界](../apps/desktop/src/features/scraper/use-review-queue.ts)。

## 存储与维护

追加迁移 [0019_review_queue.sql](../crates/infrastructure/migrations/0019_review_queue.sql)增加 `review_queues`、`review_queue_items`、`review_queue_checkpoints`。位置限制在 0–31，队列内媒体和位置分别唯一；预览绑定唯一，检查点记录禁止 UPDATE。保留资源删除后的账本以便显示失效状态。

当前 main 迁移上限为 19。账本属于应用数据库，包含在完整一致性备份中；不进入安全诊断导出。恢复数据库后仅显式读取并核验状态，不能因恢复而自动执行确认、旁车、标签或文件整理。旧应用不能读取未知未来迁移；降级使用兼容备份，见 [维护说明](maintenance.md)。

## 本轮验收要求

重新验证：重开后显式继续、确认成功但响应丢失的恢复、重复确认与过期检查点、并发 revision、资源删除/目录变化/成人重分类、取消/暂缓不写盘、返回范围保持，以及 1648×928、窄屏和键盘。新证据完成后记录在 [截图索引](../screenshots/README.md)，不以 Mock IPC 替代原生或真实来源验收。

## 验收记录

实时合并与固定侧栏本次通过前端 50 文件 / 428 项测试、lint、typecheck 和生产构建；另通过发布工具 24 项测试、beta.4 版本预检与许可证清单校验。第三方依赖未变化，Cargo.lock 仅更新第一方版本；核对完整差异后同步清单锁文件摘要，保留全部第三方材料。

原队列实现 `c81e6ec` 的后端记录为 Rust 343 项、Clippy 与格式检查通过。本次前端调整未重跑本地 Rust 或原生安装包，不能以该历史记录替代新发布流水线的结果。

当前浏览器场景、截图与数据限制见 [队列与实时合并验收](../screenshots/review-queue-2026-11-04/README.md)。测试 IPC 只验证界面和命令顺序，不代表完整原生宿主、真实 Provider 或系统 keyring 已通过。用户已授权递增至 beta.4；打包和产物验收由新 tag 的发布流水线执行，不等待打包完成。
