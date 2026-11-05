# Shared media output

## Kodi NFO 导出（2026-11-05）

电影以 [Kodi 官方电影 NFO 规范](https://kodi.wiki/view/NFO_files/Movies) 为准。导出实际已有的演员姓名、角色、顺序、头像、导演、编剧、类型、制作方、国家、上映日期、标语、地区分级、评分与票数、电影系列及来源 ID；缺资料时省略，不生成观看进度或虚构媒体信息。手改与锁定字段优先，既有根级扩展 XML 保留。

- **HTTPS 链接**：NFO 写入来源提供的海报、背景和演员头像 HTTPS 引用，不导出图片文件。
- **离线图片**：将真实图片与 NFO 一起导出，用相对路径引用；海报和背景采用 Kodi 推荐的 `<视频名>-poster.<扩展名>`、`<视频名>-fanart.<扩展名>`，演员头像采用同目录下的唯一名称。扩展名与真实图片编码一致，避免同目录多影片或嵌套目录同名文件冲突。特殊文件名不能安全用于图片引用时，图片名使用确定性的安全别名，视频和 NFO 文件名不变。

目录支持递归子目录，包括 `电影/无敌破坏王/无敌破坏1/正片.mp4` 与第二部的同名文件；输出分别位于各自视频目录。已有有效 NFO 默认直接读取，不再自动刮削。用户主动再次刮削并确认导出后，以 `update_nfo` 快照更新同名 NFO；媒体视频始终只读。已有图片相同则复用，不同则保留旧图并为新图添加内容摘要后缀，NFO 引用实际新名称。完整预览显示所有实际导出文件。切换模式释放旧预览，迟到结果不能执行。

离线导出优先验证现有图片和缓存，缺图才通过已启用来源的受控网络准备，不自动配置 Token、不把 Token 发到公共图片站点。不读取越界或符号链接图片，不把坏缓存、下载失败或超限当作离线完成。单图上限 8 MiB、图片及 NFO 总量 32 MiB、离线演员头像最多 200 张，超限返回错误而非截断。

更新 NFO 时先捕获原文件到同目录 `.mediaatlas-nfo-<预览ID>.backup`，核对捕获的原字节，再以不覆盖并发文件的方式发布新 NFO。原件留存用于恢复；中断后须由用户继续明确执行，自动流程不会在缺失原路径时接管尚未完成的更新。图片先于引用它们的 NFO 发布。所有校验仍限制在原媒体目录能力内。

本轮仅静态修改并补写序列化/回读、两种模式、切换预览、文件保护、恢复、中文嵌套目录与同名文件隔离回归；未运行测试、lint、构建、格式化或安装包。后续阶段记录中的历史验证不代表本轮已经通过运行验收。

## 当前 NFO 流程（2026-11-04）

首次扫描优先读取任何来源的有效本地 NFO，并把资料及外部标识写入数据库索引；已有可用 NFO 的条目不重复调用 Provider。缺失 NFO 时才进入自动刮削，可靠匹配或手动确认后自动生成缺失 NFO。已确认资料的库内条目在更新时补生成。既有 NFO 默认保留，解析失败不静默覆盖；源媒体仍只读，NFO 失败与资料识别结果分开。

自动 NFO 复用受限输出服务，只允许生成缺失 NFO，不写图片、替代版本或音频标签；原子发布、路径限制、文件身份和成人授权校验仍适用。其余手动图片/旁车输出保留预览和明确确认。下面各阶段记录描述这些底层保护机制；其中“仅显式 execute”不再表示自动 NFO 需要第二次用户点击。

自动输出结果单独记录在 `automatic_nfo_exports`，用于即时显示 NFO 成功/失败，不在确认过程中改动源文件清单或放宽核对凭据。下次真实扫描按普通本地 NFO 导入索引；新增源证据可以要求重新核对，但不能重放已经完成的确认。剧集系列与单集的 NFO、字段和标识分别归属；多个系列混放且无法证明目录归属时拒绝生成共享 `tvshow.nfo`，显示独立输出失败。

### 历史验证（2026-11-04）

2026-11-04：`cargo test -p mediaatlas-infrastructure` 通过 251 项单元测试和 85 项集成测试；scanner 11 项通过。Clippy（两个 crate、全部 targets、`-D warnings`）、`cargo fmt --all --check` 和 diff 检查通过。

覆盖第三方 NFO 首次建库/重复更新无 Provider 请求，剧集层级与稳定身份，自动匹配/手动确认后生成，已有与大写 NFO 原字节保留，多版本、损坏/空文件、删除后重建、输出失败与恢复、迟到文件冲突、取消/目录撤权/成人权限和核对队列不重复执行。

这些测试在 Linux 临时数据库和合成媒体上运行，不代表真实在线 Provider 或原生 macOS/Windows 验收。附加尝试 `cargo check -p mediaatlas-desktop` 被当前主机缺少 `glib-2.0.pc` 阻断；没有声称原生宿主编译或打包通过。前端本轮 455 项及构建、双平台窗口配置/schema 与浏览器截图见 [桌面控件验收](desktop-ui-controls.md)。

## Shared output implementation — Phase 4

`MediaOutputService` provides persistent `preview → explicit execute → result` for NFO and selected poster sidecars. Phase 4 supports `preserve` only. It never moves, modifies or deletes source media. Automatic missing-NFO execution now follows committed metadata confirmation under the policy above; other filesystem output remains a separate explicit action. Output payloads and expected before-image hashes are immutable SQLite snapshots associated with `file_operations`.

Existing sidecars are protected in full: their original bytes, comments, unknown XML attributes/elements, and local manual values are retained. NFO preview validates UTF-8/XML and rejects DTD, entities, oversized files and unsupported document roots. Existing valid NFO is displayed as old=new and is never rewritten. An actual parse/merge/serialize function preserves unknown subtrees and attributes, allows explicit protection of known fields, and adds absent known fields. When enrichment adds fields to an existing NFO, Preview includes a separate content-addressed `<name>.mediaatlas-<hash>.nfo` revision; execution creates that alternate only. Existing manual values are protected in the revision too. New NFO includes escaped known fields and external IDs, with manual/locked database values taking precedence. Replacing values in the original NFO requires a later explicit merge/editor workflow. Content-addressed alternate revisions allow future exports without overwriting user files.

Filesystem access uses `cap-std` directories with `cap-fs-ext` no-follow ancestor traversal, no-follow final reads, immutable backend-selected filenames, and atomic create-new hard-link publication. Targets are never overwritten. A file introduced or edited after Preview causes conflict; a pre-crash sidecar already equal to the immutable payload is recognized during explicit retry. A crash leaves `running` operations `interrupted` after recovery; recovery never performs filesystem writes. Temporary files use unique names and are removed on ordinary failure. Source identity snapshots include opened-file length, nanosecond modification time and Unix device/inode; execution (including successful retries) reopens the source without following symlinks and rejects missing/replaced files. On Windows, all source ancestors and the source file are opened through no-follow capabilities without FILE_SHARE_DELETE and held in a process-local lease, preventing rename/delete/replacement while a preview is valid. No low-64-bit ReFS identity or path-reopening fallback is used. Leases last ten minutes, are limited to 32 previews, and are released on execute completion/failure, explicit discard, UI replacement or unmount; expired/evicted/discarded plans require a fresh Preview. After restarting Windows, old plans fail closed until re-previewed; same-session create/execute is supported; completed idempotent retries verify source metadata and sidecar payloads without performing writes, while failed plans need a fresh preview after their lease is released, while cross-process recovery of a prior Windows plan is intentionally unsupported. This platform adapter has not been run on Windows in this Linux session, so Windows runtime acceptance remains pending. Other non-Unix/non-Windows targets fail closed. Existing destinations need no overwrite backup because P4 always preserves them. This deliberately avoids a broad destructive overwrite/rollback implementation before P7. Filesystems without atomic hard-link support fail closed and retain the plan for review.

Images download only through the shared P3 `NetworkEngine`, capped at 8 MiB compressed input. Content-Type must match a PNG/JPEG/WebP signature; decode applies 8192 dimensions, 20 million pixels and an 80 MiB decoder allocation budget. Decoded thumbnails are 240×360 maximum. SHA-256 filenames deduplicate identical bytes from multiple URLs; URL mappings avoid repeated downloads. Cached files and hash references are revalidated before sidecar export; successful operation retries also verify on-disk payloads, and every preview/execution rechecks current ordinary-media isolation and Root association. HTML, MIME mismatches, corrupt images and unsupported formats fail without export. Cache is supplied by the backend application directory and uses the same capability path protections. A process-wide two-permit semaphore limits image decoders across independent service instances. Network image decoding and thumbnail creation run in one blocking task and reuse the decoded image; cancellation releases the caller while the task retains its permit until finishing.

Mock integration tests exercise temporary SQLite records, exact unknown-field NFO preservation, explicit create/write records, retry and interruption, before-image conflicts, source-video preservation, symlink/ancestor replacement confinement, MIME/signature rejection, thumbnail creation, URL caching and content hash deduplication. Tests also cover selective XML merging, explicit manual protection, alternate revisions and isolation changes. These tests establish local behavior, not live provider availability or native GUI acceptance.


## Phase 5 independent adult output

Ordinary output still rejects AdultVideo/isolated/adult-root records and adult image jobs. A separate backend-only `AdultOutputAccess` revocable grant constructs `new_adult`; the channel persists in immutable plans and rejects cross-channel execute/discard. Adult plans require current AdultVideo kind, isolation flag and enabled adult-only roots. Lock revokes grants, rejects subsequent publication and releases revoked Windows preview leases; each image/cache/publication rechecks authority. Adult cache is separate, including selected cover/back-cover/sample files and bounded thumbnails. NFO exports catalog/namespace/studio/label/series/provider-scoped people without Movie matching. Existing sidecar protection, source identity and Preserve semantics remain in force.

Adult batches hold at most 32 immutable media/preview bindings and persistent per-item results. Only explicit execute writes, cancellation is shared across independent IPC service instances, unstarted files are not published, and unknown or ordinary task UUIDs cannot cancel unrelated work. Restart records interruption without automatic replay. Native Windows behavior remains pending real-device validation. See [adult contract](phase-5-contract.md).


## Phase 6 audio tags and Music consumption

`MusicTagService` is an independent audio workflow: explicit Preview → confirmation → full original-byte snapshot → verified quarantine → no-clobber tagged-file publication → persistent result. Whole-file recovery has its own Preview/confirmation, validates the original snapshot hash, rejects unrelated external edits, and retains the replaced tagged bytes too. Source/root/file/linked graph authority and selected-field protection are rechecked under the SQLite writer reservation. No scan or Provider search writes audio. Source bytes are capped at 256 MiB, at most two audio operations run concurrently, preview capabilities actively expire at ten minutes, and restart never replays mutation. Plans/payloads/backup bytes are immutable; backups remain until an explicit future cleanup policy.

Unix quarantine uses rename-with-NOREPLACE and current-path/held-parent identity verification. Windows holds no-delete directory/source leases, releases only its own source handle for a safe `atomicwrites::move_atomic` no-replace move, and checks actual quarantined content before publication. Full SHA-256 plus length/times/creation time and supplementary safe-handle identity signals protect bytes; this is not a claim of validated ReFS 128-bit object identity. An intervening external file is never overwritten; conflicts retain original SQLite bytes and on-disk quarantines for explicit recovery. Windows runtime is pending.

Actual generated MP3/FLAC/M4A/Vorbis/Opus/WAV fixtures pass Lofty read/write, unknown fields, pictures, codec essence, byte backup/restore and idempotency tests. An additional native opaque binary ID3 frame survives. Independent development-only FFmpeg decode PCM hashes match the original fixtures for all six actual service outputs. These are fixture-specific results, not a claim about every codec/profile/tag variant. See [Music contract](phase-6-contract.md).

Music Library/Artist/Album pages consume saved results and request native system opening only on a user click. Search/import, selected cover acquisition, track-file linking, tag changes and recovery live in `/music/tools`. Selected covers and source/rights notices persist for network-free Library reads; they use the same global two-decoder image budget/cache/hash rules. Native opening accepts a backend-resolved authorized audio `file_id`, validates capabilities and current file identity, and dispatches through the maintained `open` crate with a bounded launcher check. It returns only “requested”, never “playback started”; actual OS association testing remains pending.


## Phase 7 organization and recovery

`OrganizationService` is independent from preserved NFO/image export and Music tag mutation. Its immutable plan snapshots the complete selected video file set, exact Part/stem subtitle and sidecar associations, target naming, cross-platform collisions, Root/metadata scope, streaming SHA-256 source identity and existing ancestor identities. Shared generic assets in a folder containing unrelated media require review; files already owned by another media item cannot be silently taken. Preserve is default. Copy and Hardlink create destination-only owned stages; Hardlink unsupported/cross-volume failure never silently falls back to Copy. Move completes and verifies the destination copy, then no-replace quarantines each original and validates the actual moved object before updating indices. Local artwork/metadata location references move with their file mapping.

Persistent intent and prepared ownership records precede filesystem publication. A SQLite writer reservation revalidates current authority and file associations while actual changes and index updates occur. Filesystem/SQLite are not an atomic pair: a crash may leave published targets or quarantined originals while SQL still has the before-state. Recovery records Interrupted only and requires a fresh explicit rollback preview; actual journal identity/hash reconciles owned destinations, and external changes are never overwritten. Rollback itself freezes current database/ancestor state, quarantines its actual owned destination before validation, and restores originals only through no-clobber moves. If an external object was moved in a narrow replacement race, it is only returned by no-replace; an intervening new path is preserved and the displaced bytes remain quarantined.

Previews run within a 60-second filesystem budget, at most 4096 files/16384 directory entries, 32 ten-minute sessions. Execution runs on bounded blocking workers and checks cancellation/live adult grants per 64 KiB stream chunk. Independent IPC callers share typed CAS and cancellation state; cancellation acknowledges the worker has stopped. Windows supports same-session writes through held ancestor/source leases and the maintained no-replace atomicwrites adapter; real Windows/macOS behavior remains pending. App-private `.mediaatlas-`/`.mediaatlas_` recovery directories are excluded from scan/watch input and are retained until an explicit future cleanup policy. See [organization contract](phase-7-contract.md).
