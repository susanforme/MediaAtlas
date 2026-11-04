# 媒体库与扫描 IPC 契约

此处保留浏览与扫描的版本化基础契约，后续字段和服务以实际类型及追加迁移为准。模型与 JSON 形状定义在 `crates/application/src/library.rs`，扫描内部契约在 `crates/media-scanner`；不允许各层另造同名不同字段的结构。

当前默认流程见 [自动媒体库](automatic-library.md)，界面范围见根 [design.md](../design.md)。`scan_start` / `scan_confirm` 保留用于旧任务兼容；默认目录操作使用 `library_sync`，不要求手动确认入库。

## JSON 与错误协议

P0 `get_app_info` 不变。P2 新命令统一返回：

```json
{
  "schema_version": 1,
  "data": {}
}
```

`data` 随命令可以为对象、数组或布尔值。Rust `ApiResponse<T>::new(data)` 固定版本；JS 调用使用 `invoke(command, { input })`，input 内部字段为 snake_case。前端拒绝未知 schema，不将空对象强转为成功。

失败沿用 `{schema_version,code,message,correlation_id}` rejection envelope；code 为稳定英文 ID，UI 选择本地化文案，不显示原始数据库/文件系统异常。UUID 在 JSON 中为字符串，UTC 时间为 ISO/RFC3339 字符串，缺失 Option 为 null。

## Root、浏览与详情 DTO

```text
RootInput {
  id?: UUID, name: string, path: string, kind: MediaKind|null,
  enabled: bool, exclude_patterns: string[]
}
RootView = LibraryRoot 的平铺字段 + {
  exclude_patterns: string[], last_scan_at: UTC|null
}
RootListInput { include_adult: bool=false }
RemoveRootInput { root_id: UUID }

BrowseQuery {
  root_id?: UUID, kind?: MediaKind, query?: string, health?: string,
  genre?: string, person?: string, studio?: string, year?: i32,
  sort: string="updated_desc", cursor?: string, limit: u32=50,
  include_adult: bool=false
}
MediaCardView { media: MediaItem, poster_uri?: string, health: string[] }
MediaPageView { items: MediaCardView[], next_cursor?: string }
MediaDetailInput { media_id: UUID, include_adult: bool=false }
MediaRelationView { id: UUID, kind: MediaKind, title: string }
MediaDetailView {
  media: MediaItem, files: MediaFile[], artworks: Artwork[],
  external_ids: ExternalId[], metadata_values: MetadataValue[],
  relations: MediaRelationView[], health: string[], poster_uri?: string
}
LibrarySummaryView {
  total_media: u64, total_roots: u64, pending_tasks: u64, failed_tasks: u64,
  missing_poster: u64, missing_nfo: u64, incomplete_metadata: u64
}
FacetOption { value: string, count: u64 }
LibraryFacetsView {
  genres: FacetOption[], people: FacetOption[], studios: FacetOption[], years: FacetOption[]
}
```

Rust RootView 通过 `#[serde(flatten)] root: LibraryRoot` 实现平铺；不存在嵌套的 JSON `root` 属性。Root 的 path_style 由后端当前平台选定，不要求用户在 UI 猜测平台。RootInput.kind=null 表示用户显式选择 Mixed，UI 不默认选择 Mixed。

BrowseQuery 的 sort 只接受 `title_asc`、`year_desc`、`added_desc`、`updated_desc`；health 只接受 `missing_poster`、`missing_backdrop`、`missing_nfo`、`incomplete_metadata`、`duplicate` 或 null。limit 1..=200。未知值返回稳定输入错误，不能无声改成默认过滤。

cursor 是不透明 JSON 字符串；包含排序边界及过滤指纹，服务端必须拒绝在另一组过滤/排序条件下重用。翻页以数据库读取值构建游标，不使用前端重新计算时间。Root 用 EXISTS 关联文件，不能因多 Part 或多个文件使媒体重复返回。

poster_uri 只能表示已校验的本地应用缩略图/可访问缓存，不使用未校验源文件路径或远端图片。无图片为 null；实际可读取的本地图片在应用缓存生成缩略图，不改变源图片。

所有读操作都使用同一 adult 可见性规则，包括 Root、列表、详情、统计、Facet、任务和预览；默认 false。UI 开关不是认证机制，但后端不能只依靠前端隐藏成人卡片。

## 扫描与任务 DTO

```text
ScanTaskView {
  id: UUID, root_id: UUID, state: TaskState,
  visited_files: u64, skipped_files: u64, candidate_count: u64,
  imported_count: u64, warnings: u64, error_code?: string,
  created_at: UTC, updated_at: UTC
}
ScanCandidateView {
  id: UUID, root_id: UUID, relative_path: string, size_bytes: u64,
  modified_at?: UTC, kind: MediaKind, title: string, year?: i32,
  part?: u32, series_title?: string, season_number?: u32,
  episode_number?: u32, absolute_number?: u32, catalog_number?: string,
  needs_review: bool, warnings: string[], existing_media_id?: UUID,
  status: string
}
StartInput { root_id: UUID }
ListTasksInput { include_adult: bool=false }
PreviewInput { task_id: UUID, cursor?: string, limit: u32=50, include_adult: bool=false }
PreviewPageView { task_id: UUID, items: ScanCandidateView[], next_cursor?: string }
ConfirmInput { task_id: UUID, candidate_ids: UUID[] }
ConfirmResult { imported: u64, unchanged: u64, needs_review: u64 }
CancelInput { task_id: UUID }
```

TaskState 使用 P1 已有的 snake_case 值。候选 status 固定为 `pending`、`unchanged`、`imported`；warnings/error_code 为稳定代码而非任意内部异常文本。候选分页 limit 1..=200，游标绑定任务；不把全部扫描结果作为单个 IPC 消息返回。

ConfirmInput 每次最多 200 个不重复、属于指定任务的候选 ID。选择并提交视为对这些候选的显式确认，包括有 needs_review 标记的候选；界面必须展示不确定原因。needs_review 返回本次确认中的待审候选计数，不等于可绕过路径/身份验证的许可。

启动、取消、确认、Root 移除之间须维护合法状态和事务边界。任务计数来自持久化记录，不依赖 UI 事件累计；重复确认返回幂等结果。应用启动标记遗留 Running 为 Interrupted，不自动重放文件操作。

## 命令表

| 命令 | input | data |
| --- | --- | --- |
| `library_list_roots` | RootListInput | RootView[] |
| `library_save_root` | RootInput | RootView |
| `library_remove_root` | RemoveRootInput | bool |
| `library_list_media` | BrowseQuery | MediaPageView |
| `library_get_media` | MediaDetailInput | MediaDetailView |
| `library_summary` | BrowseQuery | LibrarySummaryView |
| `library_facets` | BrowseQuery | LibraryFacetsView |
| `scan_start` | StartInput | ScanTaskView |
| `scan_list_tasks` | ListTasksInput | ScanTaskView[] |
| `scan_get_preview` | PreviewInput | PreviewPageView |
| `scan_confirm` | ConfirmInput | ConfirmResult |
| `scan_cancel` | CancelInput | ScanTaskView |

命令层仅适配请求/响应和事件，不在 Tauri command 中实现扫描或直接拼 SQL。系统核心可脱离桌面运行和测试。

## Scanner 内部 API

扫描器为独立 crate，不依赖 Tauri、SQLite 或前端。基础接口如下：

```text
run_scan(
  root: &Path, options: &ScanOptions, cancel: &AtomicBool,
  on_entry: impl FnMut(ScannedFile) -> bool,
  on_progress: impl FnMut(ScanStats)
) -> Result<ScanStats, ScanError>

ScanOptions {
  kind: Option<MediaKind>, path_style: PathStyle, exclude_patterns: Vec<String>,
  known_files: HashMap<String, FileStamp>
}
FileStamp { size_bytes: u64, modified_at: Option<DateTime<Utc>> }
ScanStats { visited_files, skipped_files, candidate_count, warnings: u64, cancelled: bool }
ScannedFile {
  relative_path, normalized_relative_path, size_bytes, modified_at,
  parsed: ParsedMedia, local_metadata: LocalMetadata, artworks: LocalArtwork[],
  associated_files: string[], unchanged: bool,
  sidecar_fingerprint: string, exclude_patterns_snapshot: string[]
}
ParsedMedia {
  kind: MediaKind, title: string, year?: i32, part?: u32,
  season_number?: u32, episode_number?: u32, absolute_number?: u32,
  series_title?: string, catalog_number?: string,
  group_key: string, needs_review: bool, warnings: string[]
}
LocalMetadata { fields: BTreeMap<String, Value>, external_ids: LocalExternalId[] }
LocalExternalId { provider_id: string, namespace: string, value: string }
LocalArtwork { relative_path: string, kind: ArtworkKind }

parse_filename(&str, Option<MediaKind>) -> ParsedMedia
read_nfo(&Path) -> Result<LocalMetadata, ScanError>
```

FileStamp 时间归一 UTC 毫秒，known_files 的 key 为规范化相对路径。group_key 包含目录、类型、标题/年份等身份；TV 按单集而不是整个 show 分组，Part 可以合组，不确定项不跨不相关文件名自动合并。回调返回 false 或取消信号都能结束遍历；应用服务负责在后台线程运行 scanner 并把回调结果持久化。

## 事件与版本

基线任务事件使用 `task-created`、`task-updated`、`task-log`、`task-needs-review`、`task-completed`。事件需要版本、任务标识和可重查的状态，不发送完整候选集合。UI 将其作为刷新提示；从数据库读取是权威状态。具体事件载荷在宿主接线时与本记录同步，不把内部 Tokio/SQLx 类型暴露给 UI。

变更共享字段前先通知所有实现者并更新本文件，禁止 UI 与后端各自猜测命令名或字段形状。

### 实施补充

- `sidecar_fingerprint` 与排除快照为后端内部持久化字段，不作为 UI 大字段返回；用于识别 NFO/图片/字幕的新增、修改和删除，并在确认前复核。
- 任务事件载荷为 `{schema_version:1,correlation_id:task_id,data:ScanTaskView}`；Adult/Mixed Root 不广播明细，显式开启后的过滤查询/轮询仍可恢复进度。
- 0004 为媒体索引增加内部 `adult_isolated` 标记：Adult Root 的条目在 Root 移除或重新分类后仍保持隔离，避免保留的逻辑条目进入普通媒体墙；不会自动解除该标记。
- 所有扫描和查询写入仅限 SQLite 与应用缩略图缓存；确认操作会清理已失效侧车索引，不删实际源文件或人工选择的图片。
