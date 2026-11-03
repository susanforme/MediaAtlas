# Phase 2 实施与验收记录

- 日期：2026-11-03（Asia/Shanghai）。
- 状态：**功能实现和本地自动化验收通过；原生 GUI 与远端跨平台结果待验收**。
- 范围：P2 本地扫描与媒体库、语言 `zh-CN`/`en-US`；提交并推送后停止，不进入 P3。
- 原 P0/P1 平台待验收事实保留，按用户明确启动 P2 的指示继续；不将顺序例外回填为历史验收成功。

## 任务完成情况

| 任务 | 产物与行为 |
| --- | --- |
| P2-01 Root 管理 | `infrastructure/src/browse.rs`、`0004_root_settings.sql`、前端 `features/library/pages.tsx`：添加/编辑/启停/移除、路径校验、显式类型和排除 glob；移除不删除源文件 |
| P2-02 扫描与增量 | `crates/media-scanner`、`infrastructure/src/scan_service`：后台只读遍历、过滤、毫秒时间/大小指纹、侧车增量签名、重复扫描幂等；无 Watcher 或在线请求 |
| P2-03 类型解析与关联 | scanner 的 `parser.rs`：Movie 年份/Part、TV SxxExx/1x01、Anime 绝对集数、JAV/FC2 番号；不确定项需确认；SQL 保存 Part 归组和 Series/Season/Episode 关系 |
| P2-04 本地数据 | scanner 的 `local.rs`：NFO 字段/ID、DTD/实体拒绝、2MiB 上限；lofty 只读音频标签、禁读内嵌封面；本地元数据优先，用户锁定值保留 |
| P2-05 预览确认 | `0003_scan_sessions.sql`、ScanService：候选独立暂存、分页、每批最多 200 个显式 ID、二次确认入库；确认前复核 Root 配置、源文件与侧车；重复确认幂等 |
| P2-06 任务闭环 | 持久化计数/状态、后台线程与有界通道、进度事件、取消；启动将遗留 Queued/Running 标记 Interrupted，不重放；事件丢失用查询/轮询恢复 |
| P2-07 真实媒体库 | Dashboard、海报墙/列表、详情、任务中心；服务端 Root/类型/文本/健康/Facet/排序过滤及 keyset 分页；无数据时空状态，无假媒体/海报 |
| P2-08 IPC/缓存/隔离 | `application/src/library.rs`、桌面 commands/state：版本化接口，校验本地缩略图，普通/成人缓存分离；默认成人隐藏，关闭清除前端缓存；Root 移除后仍保留隔离 |
| P2-09 国际化 | `i18n/index.ts`：只支持 zh-CN/en-US，旧 en 迁移、持久化、en-US fallback、日期数字格式化；测试表单/错误/确认/语言切换 |

前置设计已写入 `library-workflow.md`、`ui-spec.md` 和 `phase-2-contract.md`，DTO 先冻结再分工。设计、scanner、扫描服务、查询和前端由 subagent 分工；额度中断后保留现有工作，重新分配恢复任务，由主代理完成接线、审查、集成和提交。

## 实际验证

| 命令/检查 | 结果 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、22 项 Vitest 测试及 Vite 生产构建通过 |
| `cargo test --locked -- --nocapture` | 默认核心/scanner/数据库全部 57 项通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 包含桌面宿主的完整 workspace 通过 |
| `cargo test --workspace --locked --quiet` | 原生宿主编译/链接及全部 57 项测试通过；宿主自身没有新增独立用例 |
| `cargo fmt --all -- --check` | 通过 |
| `pnpm --filter @mediaatlas/desktop tauri build --debug --no-bundle` | 成功生成 Linux 调试可执行文件；不打包、不发布 |

Rust 测试构成：application 4、bridge 2、domain 13、logging 1、浏览查询 6、P1 Repository 10、扫描服务 9、任务存储 3、scanner 9，共 57 项。前端共 7 个测试文件、22 项，覆盖原诊断协议和新增语言、隐私、Root 表单、扫描分页/勾选/确认/取消。

真实临时文件和数据库集成测试覆盖：扫描→候选暂存→分页→分批确认→媒体查询；重复扫描/确认；Part 和季集关系；人工字段保护；源/策略变更拒绝；符号链接替换；取消和 Interrupted 恢复；侧车新增/变更/删除；成人/Mixed 数据隔离；实际 PNG 缩略图与越界图片拒绝；只移除索引而保留源文件。

性能证据：1000 条媒体在四种排序下分页遍历不重不漏；同目录 10,000 视频 + 10,000 NFO 的只读 scanner 测试本轮约 **0.81 秒**，取消测试能够提前终止。该数字仅为当前 Debian x86_64 临时目录结果，不代表 Windows 磁盘或所有真实库性能，也不包含用户确认导入 10,000 条的耗时。

本地工具：Rust 1.90.0、pnpm 12.17.0、Node v24.19.0。完整原生检查沿用临时 GTK/WebKit sysroot，默认 core/scanner 测试不依赖桌面库。没有为日常 CI 增加打包，main/PR 仍只跑跨平台测试，只有 tag 才触发 Release 构建；本轮未创建 tag 或发布应用。

## 收尾修复

- 单目录重复发现侧车从 O(n²) 改为有界目录索引；排除规则也用于 NFO/图片读取和确认签名。
- Adult Root 的番号解析不再被季集规则抢先匹配；Mixed 中强番号保守分类 AdultVideo 并待审，不覆盖用户显式 Movie Root 类型。
- 本地 NFO/图片删除后，只有确认增量候选才清理失效侧车索引；不删除视频/音频或人工选择的图片。
- 普通 Root/媒体/详情/Facet/统计和任务读取统一隔离成人数据；Mixed 成人任务计数不进入普通统计。
- Adult Root 移除或重新分类不会使保留的逻辑媒体记录泄漏到普通墙；内部隔离标记不自动解除。
- 关闭 SQL statement logging，避免逐文件查询日志噪声；不记录原始 IPC/扫描 payload。

## 边界与待验收项

- 没有实际 Windows/macOS 运行或远端 CI 成功证据；历史原生 GUI 启动待办保留。当前环境 Chromium 网页截图尝试超时，未取得截图，不能将界面构建或 jsdom 交互测试称为原生端到端验收。
- Root 当前通过目录文本输入；没有新增原生目录选择插件。首次使用应指向明确的本地媒体目录，Mixed 需显式选择。
- 路径身份仍为显式平台词法规则；不实现完整 Windows Unicode/短文件名或媒体内容识别。Mixed 无法识别所有成人内容，用户须正确分类 Root 并检查待审项。
- 增量基于大小、毫秒 mtime 和侧车签名，不是完整视频内容哈希；删除/移动主媒体的高级监控与健康修复属于 P7。
- 侧车共享路径的文件索引归首个作品，图片可通过独立来源关联多个作品；自动匹配不确定时需审查，在线候选和完整类型业务仍在 P4/P5/P6。
- P2 扫描协调器暂与持久化/文件适配放在 infrastructure 的独立模块，application 固定稳定用例 DTO；避免反向依赖 SQLite/Tauri。后续有独立运行时复用需求时再抽取端口，不引入万能 Service/Trait。
- 没有完整 NFO 往返导出、标签写入、文件整理、在线 Provider、网络双引擎或自动更新；不存在写入源媒体的后门。

下一阶段仅规划：P3 API/HTTP-HTML 引擎、Provider 契约/注册/限流/错误分类。必须等待用户明确指示，当前不开始。
