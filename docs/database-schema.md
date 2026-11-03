# Phase 1 SQLite Schema

状态：P1 公共存储契约 v1，迁移实施前冻结。领域定义见 [domain-model.md](domain-model.md)，接口约定见 [phase-1-contract.md](phase-1-contract.md)。

## 连接与迁移

- SQLx + SQLite；所有 Repository 连接开启 foreign_keys、WAL 和 busy_timeout。数据库写入位于用户明确给定的本地路径；P1 不接入 Tauri 应用启动，不在打开 UI 时擅自创建媒体库。
- UUID 存为规范字符串；UTC 时间存为 Unix 毫秒整数。日期字段在后续类型表实施时使用 `YYYY-MM-DD`。JSON 存为 TEXT 并通过 SQLite JSON 校验。
- 存储时间精度明确为毫秒：写入丢弃不足一毫秒的精度，读取返回该毫秒对应的 UTC 时间。调用方不能用写入前的纳秒时间构造下一页游标，必须使用 Repository 返回的持久化值；负 Unix 时间按统一时间戳转换处理。
- 文件数据库测试验证 WAL；内存数据库不强求 WAL，因为 SQLite 内存模式不支持 WAL。
- 初始公共迁移和任务迁移分别追加，已发布迁移不改写；SQLx 维护 checksum/版本表。
- 查询与 SQL 只出现在 infrastructure Repository 中。领域模型不实现 SQLx trait，不向 UI 暴露 SQL 字符串。

## 公共表

| 表 | 主键与关键约束 | 删除关联 |
| --- | --- | --- |
| `library_roots` | UUID PK；原始 path、path_style、内部 normalized_path；`UNIQUE(path_style, normalized_path)`；kind 可为空表示显式 Mixed | 删除 Root 只移除该 Root 文件索引，不删除逻辑作品或磁盘文件 |
| `media_items` | UUID PK；kind、title、original_title、year、created_at/updated_at | 删除作品级联移除 files/artwork/external IDs/current metadata |
| `media_files` | UUID PK；media_item_id/root_id FK；保留 relative_path；`UNIQUE(root_id, normalized_relative_path)`；size 非负 | 两个父表外键分别级联删除索引 |
| `artworks` | UUID PK；media_item_id FK；kind、cache_path/source/provider、尺寸、内容哈希 | 删除作品只删除资产索引，不执行文件移除 |
| `external_ids` | UUID PK；media_item_id FK；`UNIQUE(media_item_id,provider_id,namespace,external_id)` | 删除作品级联 |
| `metadata_values` | UUID PK；media_item_id FK；field_key、统一 locale 键、JSON value、来源/时间/置信度、人工/锁定标记；`UNIQUE(media_item_id,field_key,locale)` | 删除作品级联 |

ExternalId 不跨作品全局唯一：一个远端实体可能关联不同本地版本或集级记录。主外部 ID 可以通过媒体+Provider+namespace 的 partial unique index 限制一条 primary；不能只有单个不带命名空间的 ID 列。

所有更新必须防止稳定 ID 的归属重绑：已有 MediaFile 不得移到另一 Root/作品，Artwork/ExternalId/MetadataValue 不得更改所属作品；不得通过更换 field_key/locale 让同一元数据 ID 指向另一个字段。Root 已有文件时，不允许直接修改 `path_style` 使已存规范路径失去一致性；Root 身份变化必须显式拒绝或通过未来受控重建流程，不隐式重写全部索引。

元数据 locale 的 None 规范化为空串，非空键按领域规则转为 ASCII 小写并拒绝歧义空格；`en-US` 与 `en-us` 不得绕过同一字段保护。不允许多个 NULL 绕过唯一约束。锁定和人工标记为 0/1；confidence 必须有限且在 0..1。Provider 更新必须在与保护条件一致的原子写入中检查现值，不采取先查后无条件覆盖的竞态流程。

P1 不实现所有 Person/Organization/Credit 的业务 Repository，也不预建全部电影/JAV/音乐扩展表。类型结构在领域中完成；对应业务阶段通过新迁移追加存储，避免有表无行为的空模块。

## 任务基础表

| 表 | 关键列与约束 | 删除关联 |
| --- | --- | --- |
| `scrape_tasks` | UUID PK；kind/state；不可变 input_snapshot JSON；created_at/updated_at | 不依赖 Root 或作品，保留任务历史 |
| `scrape_runs` | UUID PK；task_id FK；可空 media_item_id；provider_id；不可变 config_snapshot JSON；state；started_at/finished_at | task 删除级联；media 删除 SET NULL |
| `file_operations` | UUID PK；可空 task_id FK；不可变 plan JSON；state；created_at/updated_at | task 删除 SET NULL，保留文件操作意图记录 |

这些表不包含任务执行器、取消 token、IPC 事件或文件执行能力。数据层只提供插入、读取和显式状态更新，不提供修改输入快照的通用 upsert。

## 查询与索引

- `media_items(kind, updated_at DESC, id ASC)` 和 `media_items(updated_at DESC,id ASC)` 支持类型与稳定游标排序。
- `media_files(root_id,normalized_relative_path)` 唯一索引支持文件身份；`media_files(media_item_id,root_id)` 支持 Root EXISTS 过滤和作品文件查询。
- `external_ids(provider_id,namespace,external_id)` 支持外部身份关联；该索引本身不是跨作品唯一约束。
- 外键子表按 media_item_id 建索引；任务按 state 和更新时间建立必要索引。
- `list_media` 固定按 `updated_at DESC,id ASC` 排序：游标后续条件为 `updated_at < cursor_time OR (updated_at = cursor_time AND id > cursor_id)`。
- Root 过滤使用 EXISTS，避免同一作品在同 Root 有多个文件时被 JOIN 重复返回。kind/query/root 与游标条件同时生效。
- 每页 limit 为 1..=200；多取一项判断是否有下一页；next_cursor 以最后一项产生，不能把额外探测项当成下一页已读取内容。
- query 对 title/original_title 做参数化文本搜索；包含 `%`/`_` 时必须明确转义，不能把输入拼接为 SQL。
- P1 不实现 P2 的 Facets、健康统计和 UI 查询，不承诺并发写入期间的跨页快照一致性；游标由调用方在同一过滤条件下使用。

## 关键验收

1. 新库迁移成功、重复运行迁移无副作用；已发布 checksum 被修改时拒绝静默接受。
2. 每个连接的外键/busy_timeout 生效；文件数据库 WAL 生效。
3. 多 Root/多媒体类型 CRUD、外部 ID 多值、字段来源与锁定可重新读取。
4. Windows 盘符/UNC/ASCII 大小写和 Posix 大小写语义在同一宿主可测试；拒绝 `..`、绝对 relative path 和非法根目录。
5. 唯一约束不能被同一文件大小写差异或 locale NULL 绕过；FK 与级联关系符合上表。
6. 时间相同的多媒体条目可按 ID 翻页，不重不漏；多文件作品在 Root 过滤中只出现一次。
7. Provider 写入无法覆盖人工/锁定字段，多语言当前值相互独立。
8. 任务快照插入后保持不变；状态更新不执行文件操作，媒体删除不删除实际文件。
