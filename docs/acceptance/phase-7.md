# Phase 7 验收：文件整理与高级媒体库

本阶段覆盖 P7-01 至 P7-06，并落实最新“媒体库只读消费 + 当前系统关联应用打开”约束。契约见 [phase-7-contract.md](../phase-7-contract.md)。测试只使用临时 SQLite、合成媒体/旁车及 opener stub，不操作用户媒体；平台运行结果与本地自动化证据分别记录。

## 产品与数据边界

- `/library`、普通详情、`/adult`、成人详情和独立 Music Library/Artist/Album 只读消费可信元数据与本地文件关联。视频卡片主操作请求系统关联应用，详情是次操作；多版本/Part 明确选文件。无内置播放器、FFmpeg 运行依赖、转码或媒体服务。
- `/tools`、`/adult/tools`、`/music/tools` 保留未完成资源与搜索/导入、刮削、导出、关联、标签、组织和恢复。原消费 DTO/全局分页保留，SQL 在分页前应用 ready/privacy；不同查询 scope 的游标不可混用。
- 裸文件名推断即使扫描确认入索引，也不算已完成刮削。可信 ready 来源为已有 typed video/adult 成功确认快照或已确认导入的真实 NFO/音频标签；`provider_id=local` 本身不够。历史 NFO 经 no-follow 实际安全读取、合法 XML/非空标题验证回填；成人回填仅在显式 grant 下进行。无本地文件的远端 Music 图保留在工具区。
- TV 使用真实 Series→Season→Episode 关系；明确 Series/Season 元数据可以沿合法两层关系用于本地后代，容器仅在有已授权 ready 后代时展示并列出具体文件。自动生成父标题不提供完成证据。
- 所有 Open 接口仅收 `file_id`，后端重新验证当前 Root、关联、regular file、允许的媒体扩展与来源权限，保持 capability/Windows lease 到 dispatch。采用维护中的 `open` crate；返回 `requested` 仅表示已提交系统请求。
- 成人入口持久 `entry_visible` 与临时解锁分开。普通列表、统计、工具、健康和原生打开拒绝 AdultKind/隔离/AdultRoot 关联；成人独立 typed lane、锁定永久撤销旧计划，新解锁需要新预览，SQL 提交前再验 grant。

## P7 逐项实现

| ID | 实现 | 验收边界 |
| --- | --- | --- |
| P7-01 | 模板 `{title}`/`{year}`/`{part}`/`{stem}`/`{ext}`；不可变源/目标/附件/大小/冲突/文件数；跨平台非法组件、设备名、盘符/UNC、尾空格/点、大小写碰撞校验 | 不静默清洗为可能重名的路径；冲突必须重新预览 |
| P7-02 | 默认 Preserve；Move/Copy/Hardlink 真实执行，完整多 Part/字幕/NFO/图片/附件；原文件保护、无覆盖目标发布 | Hardlink 跨卷或文件系统不支持返回失败，不偷偷复制；Hardlink 修改会影响两个路径的同一字节，预览提示 |
| P7-03 | Preview→明确确认→执行→持久逐步日志；CAS 认领、跨 IPC 共享取消、当前 Root/文件/scope/目标快照复核 | 全量流式 SHA-256，固定 no-follow 目录能力；所有目标 create-new/no-replace，不用可能覆盖竞态新文件的 std rename |
| P7-04 | 回滚本身新 Preview/明确确认；operation-owned staging/quarantine、intent 与完成日志，源完整字节保留 | Filesystem/SQLite 非原子；启动只标 Interrupted，不自动重放。外部后改目标不能被回滚覆盖；冲突字节保留供显式恢复 |
| P7-05 | 成熟 `notify` 实际 native Watcher；Root opt-in、恢复配置后重建句柄并只读补扫、去抖合并、有界 queue/overflow 补扫、active scan 合并、generation 生命周期 | 只复用 P2 只读扫描候选；未确认预览阻止重复任务，不自动确认/入库/网络/写媒体；成人/Mixed Root 不监控 |
| P7-06 | 有界分页健康检查：缺资产/元数据、重复身份、离线文件；实际本地 artwork 可用性与可信缓存检查 | 检查不修改源或自动合并。修复导航到工具并重新走自己的任务/Preview；未知缓存路径不读取 |

## 文件安全与平台限制

源/目标通过 `cap-std`/`cap-fs-ext` 逐级 no-follow，冻结已有祖先身份和 Root/数据库关联。预览在 blocking worker 执行，60 秒总工作预算、4096 文件与每目录 16384 entry 上限；超限明确失败而非后台无限读取。执行使用有界 blocking worker，64 KiB 流式检查共享取消及成人 grant；目标创建与发布不覆盖既有字节。Move 先完成目标复制与完整校验，再将源以 no-replace 原子移入同目录自有 quarantine，核对实际被移动对象身份；误移入的外部替换对象只能尝试 no-clobber 放回原路径，路径被占则保留隔离字节，绝不盲删新 source。Rollback 对目标同样先隔离实际对象再验所有权。

Unix 使用成熟 rustix NOREPLACE rename；Windows 使用固定祖先无删除共享 leases 与 `atomicwrites::move_atomic` 无替换移动，同会话明确预览可写，并非禁用所有 Windows 写入。完整 hash、长度、时间/创建时间与安全句柄身份是字节保护；不宣称已验证 ReFS 128-bit object ID。能力有时间/数量限制，过期、撤销或进程重启后旧预览需重新生成。保留的 staging/quarantine 不自动清理；遇冲突不要手动删除唯一恢复副本。

实际 Windows/macOS 文件系统语义、系统关联应用、原生 WebView/IPC、实时 Provider、可信网络和 OS keyring 仍 pending。Linux fixture/stub 与浏览器 Mock 不替代这些平台验收。P5 视频 MVP 未通过的 live/平台项原样保留。

## 依赖评估

引入 `notify 8.2.1`（native recommended watcher，禁跟随目录 symlink），复用现有 bounded scanner、globset 与用户排除规则。`ignore` 默认 Git/hidden 语义会改变现有显式配置，当前无需替换遍历器；普通隐藏/Git 内容保持用户既有语义，只有应用保留 `.mediaatlas-`/`.mediaatlas_` 私有恢复目录始终跳过。`notify/file-id` 不代替已持有能力与完整字节校验。现有 SQLite/SQLx 的参数化分页足以支撑当前规模，Tantivy 暂缓，不迁移 rusqlite。新增依赖的许可证随 P8 清单核对。

## 最终检查

| 检查 | 实际结果 |
| --- | --- |
| `pnpm check` | 通过：Biome、TypeScript、150 tests / 23 files、生产构建；主 bundle 495.41 kB，无体积警告 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过：完整 workspace/desktop/测试，0 warnings |
| `cargo test --workspace --locked` | 通过：252 项功能测试，0 失败；infrastructure unit 167 项，不将 0 项 doc-test 计入功能测试 |

最终检查使用冻结后的完整实现。中间完整检查发现旧 Root Repository 的重复 UNC 冲突偶发返回 Database：Root 保存使用 deferred read→write，加上失败事务异步 rollback 会形成过期写快照。已在读取 before-image 前保留 SQLite writer reservation，并对冲突显式等待 rollback；新增 24 个并发规范化同 Root 写入回归断言 1 成功、23 Conflict，Root Repository 11 项与最终整套测试均通过。没有修改原失败断言或跳过失败测试。

组织 16 项临时文件集成覆盖四模式与 7 文件多 Part/附件、精确邻片排除、共享 generic/别属索引冲突、同路径与重复执行、源/目标/Root/祖先/scope 改变、真实 `/tmp`→`/workspace` 不同设备 Move 与回滚、跨卷 Hardlink 拒绝、Hardlink 不支持故障注入无降级、发布与实际 quarantine 对象替换竞态、same-size/还原 mtime 后 full SHA 仍拒绝、后改目标/索引回滚拒绝、local source 引用双向迁移、64 KiB copy barrier 下独立 service 取消并等待 ack、copy 中成人 lock 停止及旧 grant 不能复活。四 journal 边界（prepared/published/source quarantine/index commit）使用确定性故障注入与 Interrupted 状态恢复验证；这不是实际断电或强杀进程实测，不将特殊文件系统不支持模拟描述为所有本机文件系统已验收。

Watcher 10 项使用 Linux 实际 native event、debounce、未确认预览合并、bounded overflow、旧 generation、async shutdown 确认、Root 禁用/重分类/移除/磁盘目录置换以及停机期间修改后只读 catch-up；零自动入库。健康 3 项验证分页/隐私、离线文件、重复身份与实际 local/可信 cache 图片缺失，不读取不可信数据库缓存路径。原 P2 扫描 9 项保持通过。

Readiness 6 项覆盖 filename/NFO/成人回填/分页/cursor/移除 NFO 后保留已确认来源；真实 localhost TMDB-TV 适配器链覆盖 P2 扫描→TV Series 搜索/详情→明确确认→Series/Season/Episode ready→Root/facets→多集文件与逐 episode Open stub，并验证错误关系、成人 Root/中间 Season 污染拒绝。视频 Open 7 项、音乐 Open 7 项、音乐图谱 10 项通过；Provider 网络全部为本地 Mock。UI 覆盖只读消费、多 Part 选择、native 请求、迟到响应/换资源 token、工具配置冻结、Preview→明确确认→独立回滚、日志、Watcher opt-in 与显式健康检查。

真实 Chromium 浏览器 Mock 冒烟通过：`/library`、`/tools`、`/adult`、`/adult/tools`、`/music`、`/music/tools` × zh-CN/en-US 共 12 次渲染，`pageerror=[]`；普通/成人库四次点击主操作后都先显示多 Part 选择，显式点 Part2 才各提交一次仅含 `file_id` 的 typed Open 请求。额外普通/成人详情四次渲染只读；没有自动 Open/Preview/Confirm/Import/Execute/Watcher 配置，也没有 audio/video 播放器。会话证据 `/tmp/p7-browser-routes.log`，脚本保留在仓库外工具目录；这仍是 Mock IPC，不替代原生 OS 关联实测。

独立 reviewer 实读 FS/SQLite 安全窗口与 maintained `atomicwrites 0.4.4` no-replace API，并独立执行组织 14 项、Watcher 10 项、Readiness 6 项及最终 read-privacy 1 项均通过；最终全套另覆盖之后新增组织边界与 Root 并发回归。Windows/macOS 实际文件/关联应用仍 pending。

## 后续授权

用户最新明确授权完成所有任务后创建 beta tag 并产生支持平台 Release 可用包，覆盖早期不发布限制。P7 完成检查后由主代理 review/推送，再启动 P8；P8/主代理按 Tauri 社区成熟平台矩阵并行 CI 构建并最终打 tag，不要求未经支持的每个架构组合，不提前发布。
