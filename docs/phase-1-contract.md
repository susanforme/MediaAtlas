# Phase 1 共享接口冻结记录

状态：v1；在领域、SQLite、任务存储并行实现前冻结。用途是明确共享结构和方法边界；不构成新 UI/IPC 协议。

相关设计：[领域模型](domain-model.md)、[数据库 Schema](database-schema.md)。

## 公共类型

- crate 根重导出领域实体，所有实体 ID 使用 `uuid::Uuid`，时间使用 `chrono::DateTime<Utc>`。
- 精确公共实体字段以 domain-model 的公共实体、MetadataValue 和任务段为准。
- `MediaKind`、`PathStyle`、`ArtworkKind`、`TaskKind`、`TaskState`、`FileOperationState` 使用 serde snake_case。
- `normalize_root_path(path: &str, style: PathStyle) -> Result<String, ValidationError>`。
- `normalize_relative_path(path: &str, style: PathStyle) -> Result<String, ValidationError>`。
- `normalize_locale(locale: Option<&str>) -> Result<Option<String>, ValidationError>`；合法非空键统一 ASCII 小写。字段 ID 拒绝首尾空格和 NUL；存储层保存元数据前必须调用领域校验，避免等价 locale 绕过保护。
- `ValidationError` 不携带未经脱敏的用户完整路径或值；具体校验错误类别由实现列举，不能将无效输入悄悄规范化为其他身份。
- `MetadataWriteMode::{User,Provider}` 明确区分用户修改与 Provider 更新；写入成功返回 `Ok(())`，现值受保护返回 `RepositoryError::ProtectedMetadata`。

## LibraryRepository 边界

基础设施提供一个可独立创建的 SQLite Repository；它不依赖桌面启动或 Tauri 全局状态。

```text
open(path: &Path) -> Result<LibraryRepository, RepositoryError>
save_root(&LibraryRoot) / get_root(Uuid) / list_roots() / remove_root(Uuid)
save_media(&MediaItem) / get_media(Uuid) / list_media(&ListMediaQuery) / remove_media(Uuid)
save_file(&MediaFile) / list_files(media_item_id: Uuid)
save_artwork(&Artwork) / list_artworks(media_item_id: Uuid)
save_external_id(&ExternalId) / list_external_ids(media_item_id: Uuid)
save_metadata_value(&MetadataValue, MetadataWriteMode) / list_metadata_values(media_item_id: Uuid)
```

以上均为 async 方法；读取单项以 `Option<T>` 表示不存在，移除返回是否实际移除。数据库错误保留基础设施边界，面向调用者的 Display 不输出完整用户路径、SQL、凭据或数据库原文。

```text
ListMediaQuery {
  kind: Option<MediaKind>, root_id: Option<Uuid>, query: Option<String>,
  cursor: Option<MediaCursor>, limit: u32
}
MediaCursor { updated_at: DateTime<Utc>, id: Uuid }
MediaPage { items: Vec<MediaItem>, next_cursor: Option<MediaCursor> }
```

limit 1..=200；排序固定 `updated_at DESC,id ASC`。Root 通过 MediaFile EXISTS 关联；不把 Root ID 冗余写入 MediaItem，不使用大 offset。

数据库使用 Unix 毫秒，返回的 DateTime 已处于存储精度；next_cursor 必须从本页实际返回的最后一项产生，不能直接复用写入前纳秒级时间。客户端在同一过滤条件下续页；不提供并发更新期间的快照一致性保证。

save 需要约束 ownership：已有 MediaFile/Artwork/ExternalId ID 不能被更新到另一个作品或 Root 来绕过唯一性/来源。具体更新策略由实现选择显式拒绝或受约束更新，不能用 SQLite REPLACE 隐式删除再插入触发意外级联。

MetadataValue 已有 ID 同样不得改变作品、field_key 或 locale 的归属。已有文件的 Root 不得直接切换 path_style 破坏规范化身份；路径模式变更不是普通显示名称编辑。

Metadata Provider 模式的受保护判断必须与写入原子关联；用户模式允许显式修改已有值和锁定状态。不能让 Provider 模式通过传入 `is_user_modified=true` 伪造用户确认。

## 任务存储边界

任务存储使用相同 SQLite pool/迁移体系，可通过独立 Repository 暴露，不修改领域实体公共字段。

```text
insert_task(&TaskRecord) / get_task(Uuid) / update_task_state(Uuid, TaskState, updated_at)
insert_scrape_run(&ScrapeRun) / get_scrape_run(Uuid) / update_scrape_run_state(...)
insert_file_operation(&FileOperationRecord) / get_file_operation(Uuid) / update_file_operation_state(...)
```

插入使用唯一 ID 语义，不采用可覆盖 input/config/plan 的 upsert。状态更新不得修改输入快照或尝试调用文件系统。P2 才实现调度、取消和应用退出后的任务恢复策略。

## 范围与审查

- P1 不实现前端页面、IPC 新命令、扫描、网络 Provider、NFO/图片/标签写入或文件整理。
- 领域/SQLite/任务存储可按此契约独立实现；同一个领域类型只由领域实现 agent 修改。
- 类型扩展在 P1 只有独立结构及序列化测试；P1 公共存储可以保存不同 MediaKind 的 MediaItem，不表示完整扩展业务已接入。
- P0 平台验收未完成的事实保留。用户本轮明确要求开始 P1，因此执行 P1 设计与实现，不将顺序例外伪装为 P0 验收成功。
- 发现接口必须变化时，先同步本文件与其他实现者，再修改代码，避免同名不同字段的 DTO 或重复任务枚举。

### 迁移前审查记录

2026-11-03，主代理已审查本契约、领域模型和数据库 Schema v1，同意实施 0001 公共迁移和 0002 任务基础迁移。重点确认：Root 通过文件关联作品；显式平台路径身份；毫秒 UTC 游标；外部 ID 不跨作品全局唯一；Provider 原子保护人工/锁定字段；任务输入快照不可变；类型扩展只实现独立结构及序列化，不预建全部未来业务表。该审查为设计审查，不表示测试已经通过。

实现后独立审查补充：新增 PersonAlias 的独立身份关联；规范化 locale 键并拒绝歧义空格，补回归测试验证同一语言的保护不能被大小写绕过。上述补充不改变已冻结实体的原字段，不新增 UI/Provider 能力。
