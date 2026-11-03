# Phase 1 领域模型

状态：P1 共享契约 v1，先于实现冻结。基线：[执行计划第 5 节](UNIVERSAL_MEDIA_SCRAPER_AGENT_PLAN.md)。存储映射见 [database-schema.md](database-schema.md)，精确接口约定见 [phase-1-contract.md](phase-1-contract.md)。

用户本轮明确要求开始 Phase 1。P0 的平台启动和远端 CI 仍待验收，本次属于用户授权的阶段顺序例外；不把这些待验证项目改写为通过。

## 表示与依赖

- 主键统一 `uuid::Uuid`，新实体使用 UUID v4；外部 Provider ID 始终是字符串，不能转换成本地主键。
- 时间使用 `chrono::DateTime<Utc>`；发行日期使用 `chrono::NaiveDate`，避免仅有日期的数据引入时区。
- 领域实体支持 serde，字段名及枚举值使用稳定 `snake_case`。领域不依赖 SQLx、SQLite、Tauri 或 HTTP。
- 核心模型在 `mediaatlas_domain` 根重导出。模块拆分只是代码组织，不改变公共名称。
- `MediaKind` 保留基线全部变体：Movie、TvSeries、TvSeason、TvEpisode、Anime、AdultVideo、MusicArtist、MusicReleaseGroup、MusicRelease、MusicTrack、MusicRecording、MusicVideo、Concert。没有 Game/Book 等业务空壳。
- P1 只实现各类型数据结构/序列化，以及公共媒体索引的 Repository；不实现采集、匹配、合并、标签写入或媒体文件操作。

## 公共实体

| 类型 | 字段 |
| --- | --- |
| `LibraryRoot` | `id`, `name`, `path`, `path_style`, `kind: Option<MediaKind>`, `enabled`, `created_at`, `updated_at` |
| `MediaItem` | `id`, `kind`, `title`, `original_title: Option<String>`, `year: Option<i32>`, `created_at`, `updated_at` |
| `MediaFile` | `id`, `media_item_id`, `root_id`, `relative_path`, `size_bytes: u64`, `modified_at: Option<DateTime<Utc>>`, `part: Option<u32>` |
| `Artwork` | `id`, `media_item_id`, `kind: ArtworkKind`, `cache_path: Option<String>`, `source_url: Option<String>`, `provider_id: Option<String>`, `width: Option<u32>`, `height: Option<u32>`, `content_hash: Option<String>`, `created_at` |
| `ExternalId` | `id`, `media_item_id`, `provider_id`, `namespace`, `external_id`, `is_primary`, `created_at` |
| `Person` | `id`, `name`, `aliases: Vec<String>` |
| `PersonAlias` | `id`, `person_id`, `alias`, `locale: Option<String>` |
| `Organization` | `id`, `name` |
| `Credit` | `id`, `media_item_id`, `target: CreditTarget`, `role`, `character_name: Option<String>`, `sort_order: u32` |

未标注的 ID 为 UUID，时间为 UTC，文本为 String。Root 的 `kind=None` 表示显式 Mixed；不为 Root 赋予默认 Mixed。MediaItem 不绑定单个 Root，一个逻辑作品可在多个 Root 具备文件；按 Root 查询时通过 MediaFile 关联。

`CreditTarget` 为 `Person(Uuid)` 或 `Organization(Uuid)`，不同时填写两个目标。角色保存稳定 ID 字符串，避免预设不可扩展的角色全集。别名身份由独立 `PersonAlias.person_id` 关联，`Person.aliases` 仅为显示快照；同名别名不构成不同真人之间的自动合并依据。P1 定义人员、别名、组织和 Credit 结构；对应业务 Repository 随真实需求增加。

`ArtworkKind` 包含 Poster、Backdrop、Logo、Thumb、Scene、Artist、AlbumCover、Cover、BackCover、Sample。`cache_path` 是本机应用缓存引用，不是媒体文件身份；图片实际下载和校验不在 P1。

## 跨平台路径身份

`PathStyle` 显式区分 Windows 与 Posix，不依赖运行测试的宿主 OS 猜测路径语义。

- Root 保存原始绝对路径和基础设施生成的规范化身份；Windows 接受盘符根和 UNC 共享根，Posix 要求 `/` 起始。
- MediaFile 保留 `relative_path` 原文；另存规范化相对身份。文件唯一性为 `(root_id, normalized_relative_path)`。
- 规范化必须拒绝空相对路径、绝对路径、盘符相对路径以及 `..` 路径段。消去 `.` 和重复分隔符，不读取文件系统、不解析符号链接。
- Windows 相对路径将反斜杠视为分隔符并折叠 ASCII 大小写；Posix 保留大小写且反斜杠不是目录分隔符。
- 不宣称实现 Windows 文件系统全部 Unicode 比较、短文件名、符号链接或实际文件身份；这些能力在扫描的平台适配层进一步处理。P1 不将不同 Root 或仅名称相似的文件自动合并。
- 这些函数是索引身份规则，不授予文件写入权限；文件整理仍需 Preview 和确认。

## 来源、语言和锁定

`MetadataValue` 字段：`id`, `media_item_id`, `field_key`, `locale: Option<String>`, `value: serde_json::Value`, `provider_id: Option<String>`, `source_url: Option<String>`, `fetched_at: Option<DateTime<Utc>>`, `confidence: Option<f64>`, `is_user_modified`, `is_locked`。

- 同一媒体的 `(field_key, locale)` 是当前字段值身份；无 locale 的字段使用统一空串存储键，不能依赖 SQL NULL 唯一性。
- locale 使用 ASCII 字母数字、以连字符分段的键，每段 1–8 字符；拒绝空值、空段、非 ASCII 及首尾空格，存储键统一小写，因此 `en-US` 与 `en-us` 共享保护状态。该规则不宣称实现完整 BCP47 注册表或语言别名解析。`field_key` 是稳定、区分大小写的字段 ID，拒绝首尾空格和 NUL。
- `confidence` 若存在必须是有限数并落在 0..=1；不得持久化 NaN、Infinity 或超范围值。
- 用户写入和 Provider 写入必须使用显式不同模式。Provider 更新遇到当前 `is_locked` 或 `is_user_modified` 时返回受保护结果，不能默默覆盖。
- 人工编辑具有最高优先级；用户可以显式修改自己锁定的值和锁定状态。Provider 来源不能把自身字段冒充成人工修改。
- P1 只保证上述持久化与写入保护，不提前实现 P4 的跨 Provider 优先级、候选比较和完整 Merge Preview。
- 文本字段选择与图片选择独立。多语言字段使用不同 locale，保存一门语言不覆盖其他语言。

## 类型扩展模型

每种领域使用独立结构；不创建一个包含所有可选字段的万能 Metadata。以下为数据边界，具体代码字段以冻结契约与实现一致性检查为准。

| 类型 | 专属字段与关联 |
| --- | --- |
| `MovieMetadata` | 片长、budget/revenue、原语言、制作公司 ID、collection |
| `TvSeriesMetadata` | status、network、首播/末播日期、episode order |
| `SeasonMetadata` | series ID、季号、名称、简介、海报关联 |
| `EpisodeMetadata` | series/season ID、季/集/绝对集数、播出日期、片长、still、编号映射 |
| `EpisodeNumberMapping` | `EpisodeOrder`（Aired/DVD/Absolute/Anime）及对应季/集编号；不把 SxxExx 当成跨方案唯一身份 |
| `AnimeMetadata` | 基础 Movie/TV 关联、format、season/cour、studio、source、别名；AniDB/AniList/Bangumi ID 经公共 ExternalId 表保存 |
| `AdultVideoMetadata` | CatalogNumber 集合、studio/label/series、日期/片长/描述、performer/director/genre、封面/背封/样品图/trailer 关联 |
| `CatalogNumber` | 原始值、标准化值、发行商命名空间、主番号标记和别名；此阶段不实现匹配或粗暴删除标点的归一规则 |
| `MusicArtist` | Artist 身份、sort name、Alias/Credit 关联 |
| `MusicReleaseGroup` | 独立 Release Group 身份、Artist 关联、发行组类型 |
| `MusicRelease` | Release Group 关联、地区/日期/状态、Barcode/Catalog Number、Artist 关联；不能与 Release Group 混同 |
| `MusicMedium` | Release 关联、碟号、格式；Medium 是多碟专辑容器，不增加新的 MediaKind |
| `MusicTrack` | Medium/Recording 关联、曲序、时长；Track 是发行上的位置，不等于 Recording |
| `MusicRecording` | Recording 身份、ISRC、时长及 Artist 关联 |

当前独立结构的精确字段如下（`?` 表示 Option，`[]` 表示 Vec；没有复制成公共万能 Metadata）：

```text
MovieMetadata: media_item_id, runtime_minutes?, budget?, revenue?, original_language?,
               production_companies: UUID[], collection?
TvSeriesMetadata: media_item_id, status?, networks: UUID[], first_air_date?, last_air_date?, season_order
SeasonMetadata: media_item_id, series_id, season_number, name?, overview?, poster: UUID?
EpisodeNumberMapping: order, season_number?, episode_number
EpisodeMetadata: media_item_id, series_id, season_id?, season_number?, episode_number?,
                 absolute_number?, air_date?, runtime_minutes?, still: UUID?, number_mappings[]
AnimeMetadata: media_item_id, base_media_item_id?, format?, season?, cour?, studios: UUID[], source?, aliases[]
CatalogNumber: raw, normalized, namespace, is_primary, aliases[]
AdultVideoMetadata: media_item_id, catalog_numbers[], studio: UUID?, label: UUID?, series?,
                    release_date?, runtime_minutes?, title, original_title?, description?,
                    performers: UUID[], directors: UUID[], genres[], cover: UUID?, back_cover: UUID?,
                    samples: UUID[], trailer?, source_provider?, source_url?
ArtistCredit: artist_id, credited_name, join_phrase
MusicArtist: media_item_id, name, sort_name?, aliases[]
MusicReleaseGroup: media_item_id, title, artist_credits[], primary_type?, secondary_types[], first_release_date?
MusicRelease: media_item_id, release_group_id, title, artist_credits[], country?, release_date?, status?, labels: UUID[]
MusicMedium: id, release_id, position, title?, format?
MusicTrack: media_item_id, medium_id, recording_id, position, number, title, length_ms?, artist_credits[]
MusicRecording: media_item_id, title, length_ms?, artist_credits[]
MusicIdentifier: media_item_id, kind, namespace, value
```

`MusicIdentifierKind` 为 MusicBrainz、Isrc、Upc、Ean、CatalogNumber；ISRC、UPC/EAN 和 Catalog Number 经该独立结构关联，不混入影片 ID。ArtistCredit 保留发行时的署名和 join phrase，不用人员显示名称替代音乐 Credit 语义。

P1 对这些结构做序列化往返和独立类型测试；不为未实现功能创建全部扩展表或在线适配器。后续追加扩展表时保持这些身份边界，音乐业务仍在 P6。

## 任务与执行意图基础

- `TaskRecord`：`id`, `kind`, `state`, `input_snapshot`, `created_at`, `updated_at`。
- `TaskKind`：ScanLibrary、ParseFile、SearchProviders、FetchMetadata、DownloadArtwork、MergeMetadata、ExportNfo、WriteAudioTags、OrganizeFiles、RefreshLibrary。
- `TaskState`：Queued、Running、WaitingForUser、Succeeded、Failed、Cancelled、Interrupted。
- `ScrapeRun`：`id`, `task_id`, `media_item_id: Option<Uuid>`, `provider_id: Option<String>`, `config_snapshot`, `state`, `started_at`, `finished_at: Option<DateTime<Utc>>`。
- `FileOperationRecord`：`id`, `task_id: Option<Uuid>`, `plan`, `state`, `created_at`, `updated_at`。
- `FileOperationState`：Planned、Approved、Running、Succeeded、Failed、RolledBack、Interrupted。

三个 JSON 快照创建后不可通过状态更新接口修改；只允许不含原始密钥/Cookie/Authorization 的配置或凭据引用。P1 不启动调度器，不执行任何文件计划，不在启动时自动重放移动/覆盖任务。状态和记录只构成后续 P2/P4/P7 的持久化基础。

TaskRepository 递归拒绝常见凭据键，且要求快照为 JSON 对象；调用方仍须主动脱敏并使用凭据引用，不能声称该校验能识别任意文本中隐藏的秘密。P1 没有日志记录这些输入。
