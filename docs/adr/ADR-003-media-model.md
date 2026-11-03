# ADR-003：共享媒体身份，独立类型模型

- 状态：原则采用；P1 输出模型/Schema 草案并审查后实现。
- 日期：2026-11-03。
- 基线：第 5、7、8、9.6 节。

## 背景

媒体类型共享本地文件和资产管理需求，但影视季集、JAV 番号和音乐发行实体具有不同语义。一个万能 Metadata 或标题相似度模型会丢失身份、版本和来源信息。

## 决策

- 公共层统一 MediaItem、MediaFile、Artwork、Person、Organization、Credit、ExternalId 和 MetadataValue。
- Movie、TvSeries、Season、Episode 使用独立扩展；季集保存 Aired/DVD/Absolute/Anime 编号映射，不以 `SxxExx` 作为永远唯一的身份。
- Anime 关联 TV/Movie 基础实体并保存格式、季度、制作社、别名和 AniDB/AniList/Bangumi ID，不建立无法关联的孤岛。
- AdultVideo 使用独立领域、Provider 和 UI 分区。CatalogNumber 保留原始值、标准化值、命名空间、别名和主番号；详情返回后再次核验番号。发行商、重发版或其他冲突进入人工审核。
- Music 区分 Artist、ReleaseGroup、Release、Medium、Track、Recording，不套用电影结构；音乐数据契约在 P1 设计，业务在 P6 实现。
- 外部身份保存 provider + namespace + value；字段值记录来源、URL、抓取时间、语言、置信度、用户修改和锁定状态。
- 合并优先级为 `locked/user_modified > 用户选择 > Provider 优先级 > fallback`；输出字段级 Diff，不静默覆盖。人员别名作为关系保存，不仅按同名合并真人。
- 文件身份以 `root_id + relative_path` 管理，路径规则通过平台适配处理。候选结果与正式入库实体分开，用户确认后保存。
- 文件和标签操作先 Preview/确认，默认 Preserve。成人入口默认关闭，独立 Root、搜索和缩略图缓存，普通媒体视图与统计默认排除成人内容。

## 影响

每种类型有自己的 Parser、Query/Result/Metadata DTO 和 Matching 规则；共享底层身份与来源能力而不共享全部业务字段。P1 必须先完成 `domain-model.md` 和 `database-schema.md` 审查；P0 不提前生成所有媒体类型结构或假媒体数据。未来扩展仅保留明确身份边界，不实现 Game/Book 等空模块。
