# 持久化人工核对队列

当前 P0 的实现契约；本轮重新验证前端 46 文件 / 397 项测试、Rust 默认工作区 343 项测试及生产构建、Clippy 和格式检查。默认目录流程仍是后台自动发现和可靠匹配；此队列用于普通影视的手动核对，不增加入库门槛。产品主线见 [自动媒体库](automatic-library.md)及 [Emby 流程基准](emby-scraper-workflow.md)。

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

## 本轮验收结果

397 项前端测试、343 项 Rust 测试、Clippy 零警告、Rust 格式及许可证检查通过。浏览器十组场景及八张实际截图见 [队列验收](../screenshots/review-queue-2026-11-04/README.md)。这些截图使用测试 IPC，不代表完整原生宿主、真实 Provider 或系统 keyring 已通过；真实持久化与确认事务由本轮 Rust 服务测试验证。未移动 tag，不等待打包。
