# Phase 4 实施与验收记录

- 日期：2026-11-03（UTC）。基线：main `9cbb173`（P3 已提交并推送）。
- 状态：P4 实现、Mock/临时 SQLite/临时文件与前端本地验收通过；主代理独立审查已完成。实时 Provider、真实系统密钥服务与跨平台原生运行仍未验收。
- 执行：先冻结 [P4 契约](../phase-4-contract.md)，再分配 Provider、共享输出和 UI 的独立文件；主集成负责持久流程、匹配/合并、IPC 与安全边界。按本会话授权，在 main 使用 Codex 身份提交，主代理推送后开始 P5；不创建 tag/Release。

## 任务对应结果

| 任务 | 实现与证据 |
| --- | --- |
| P4-01 | TMDB 真实 API Movie 标题/外部 ID 搜索、详情、图片候选、external IDs；numeric ID 校验、语言/来源记录，默认禁用 |
| P4-02 | TMDB TV Series→Season→Episode 真实请求与身份复核；明确 Aired，拒绝伪造 DVD/Absolute/Anime 映射；季/集/图片资源超限明确失败 |
| P4-03 | 先评估 AniList/AniDB/Bangumi，选择 public read-only AniList GraphQL；保留 native/romaji/英文标题、别名、AniList/MAL ID、格式/季度/集数/工作室/图片；强制非成人 ANIME，TVDB 延后条件另记 |
| P4-04 | Movie/TV ID→标题/年份与 Anime 独立别名匹配；同名重制/年份及同 namespace 不同 ID 均不误去重；始终显式选择候选 |
| P4-05 | 不可变 Merge Preview 显示旧/新/来源/语言/保护；空值不擦除，locale 归一化，人工/锁定优先；确认事务重验字段 before-image、类型/成人隔离与 canonical 列，连毫秒时间相同的并发编辑也拒绝 |
| P4-06 | typed IPC 双语 Scraper、候选理由、Movie/TV/Anime 详情、季集、独立图片选择、受控单图缩略图、合并确认；媒体库展示已确认类型来源结构，历史恢复持久候选不重搜 |
| P4-07 | NFO 安全解析/序列化、DTD/entity 拒绝、未知子树/属性/注释保留与手工字段保护；图片 MIME/magic/字节/尺寸/像素/内存预算、全局 2 个后台解码 permit、缩略图、URL 缓存及 SHA-256 去重 |
| P4-08 | 独立 Preview→显式 execute→记录；capability/nofollow 目录访问、原子 no-clobber 发布、before-image 冲突、source 文件身份复核、失败/重启恢复及幂等；Preserve，不移动/改写/删除原视频 |
| P4-09 | Wikipedia 真实 HTTP/HTML 单篇英文文章补充适配；实施前记录条款/robots/归属和无图片许可边界，至少 1 秒/单并发，不绕登录、付费、403/429 或 CAPTCHA |
| 用户追加认证要求 | Settings 可手动保存 TMDB v4 Bearer / v3 API Key；系统密钥默认、会话存储显式选择、状态无回显、删除与启用分开；UUID revision 固化到 Job，仅实际请求注入 query secret；跨源 query-auth redirect 拒绝 |

## 实际检查

| 命令/检查 | 最终结果 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、86 tests（14 files）、生产构建通过；主 bundle 493.47 kB，无体积警告 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 全 workspace/宿主/测试通过 |
| `cargo test --workspace --locked` | 131 项测试通过，0 失败；不把 0 项 doc-test 当功能测试 |
| 主代理真实 Chromium 路由冒烟 | `/scraper`、`/providers`、`/network`、`/tasks` 在 zh-CN/en-US 渲染，0 pageerror；browser-only 正确显示需要桌面后端。脚本：`/workspace/.mediaatlas-tools/phase4-browser-routes.cjs` |
| 主代理真实 Chromium + 明确 Mock IPC 认证表单 | API-key/session 保存、密码 masked 且提交清空、无 localStorage secret、删除、AniList public 无凭据表单通过。脚本：`/workspace/.mediaatlas-tools/auth-browser-smoke.cjs`；不等于 native IPC/OS keyring 测试，未模拟 cleanup_pending 分支 |

新增 Rust 验证包括真实 Mock TMDB/AniList/Wikipedia 请求与 ID/季集拒绝，14 项 Scraper 匹配/SQLite/输出/取消/认证快照测试，13 项安全输出/图片测试，8 项认证状态/删除/切换/失败恢复测试及 query credential 重定向防泄露。原 P3/P2 测试全部保留通过。临时媒体原字节始终相同，开发未操作用户媒体。

认证异常回归验证：SQL 切换失败保留旧认证；新设置成功但旧 OS 凭据删除失败时，opaque retirement outbox 保留可重试引用，并返回真实 configured 状态与 `cleanup_pending`，不是伪报保存失败。旧引用立即禁止认证，状态刷新重试物理清理。Clear 的 SQL 失败保留原认证。秘密不进入状态、任务快照、SQLite、Network Inspector URL 或日志；System 不可用使用可控失败注入测试，不声称本机真实 keyring 已可用。

## 输出能力与设计边界

- 既有 NFO/图片全部保留，不覆盖。实际 XML parse→选择性 merge→serialize 保留未知内容；缺失字段可生成独立内容寻址 `<name>.mediaatlas-<hash>.nfo` 修订版，并在 Preview 中明确展示。原 NFO 中已有人工字段仍保留，原文件覆盖编辑留待后续明确流程。未覆盖旧文件，因此无需假设已有破坏性备份/回滚实现。
- 新 sidecar 使用受约束目录能力和原子 create-new hard-link；不支持 hard-link 的文件系统明确失败，不降级为覆盖。中断不会自动重放写入；用户再次显式执行才恢复，已落盘同内容可识别。
- Unix 保存 opened-file dev/inode、长度和纳秒 mtime，执行及幂等重试重新复核；删除、替换、symlink、禁用/更改 Root、成人隔离变化均拒绝。
- Windows 使用根/祖先/源文件 no-follow 且排除 FILE_SHARE_DELETE 的句柄 lease，避免假称 ReFS low-64-bit ID 唯一。预览最多 32 个、十分钟真实定时过期；取消、替换、卸载及执行完成/失败均释放。到期/重启/失败后需重新预览；成功重试仅校验磁盘内容，不重复写入。Windows 条件测试已写，本 Linux 会话未编译/运行 Windows 目标，此路径仍需实机验收。其它非 Unix/Windows 目标 fail closed。
- 图片在用户单图请求或确认所选图片时获取，不预拉整图库；主库使用本地缩略图，UI 不直接请求远端图片。普通缓存不会服务成人隔离条目。

## 实时与平台未验收事项

- [Provider 访问文档](../provider-access.md) 区分真实适配代码、Mock 成功和 live 验收。当前实例尚未观察到用户云端 `TMDB_PRIVATE_TOKEN` 的有效运行时注入，未要求用户在聊天重发值。兼容原 `TMDB_READ_ACCESS_TOKEN`，bootstrap 仅在不存在 Settings 配置时创建新的 UUID 会话引用，绝不覆盖用户保存方式、写入 keyring 或重绑历史 Job。
- P3 对未知环境代理保持 fail closed；没有因为环境变量、Mock 成功或公开 API 而放松 SSRF、DNS、TLS 或重定向保护。实际 TMDB/AniList/Wikipedia/图片外网授权及可用性未验收，Fixture 不冒充 live 成功。
- Linux 原生编译/链接通过，但已知 rootless sysroot 缺少 WebKit 硬编码系统 helper 路径，实际 native GUI/IPC 端到端仍未验收；未改代码绕过沙箱。Windows/macOS 启动、真实 OS 凭据写删、远端 CI、安装包和发布均未验收。
- Windows 本地库与认证 API 支持不以 Linux 检查冒充实机结果；跨平台输出/密钥与原生验收在后续阶段继续追踪。

## P5 接续

复用当前任务/请求记录、字段保护、不可变候选/预览和共享输出。AdultVideo 仍被普通 Scraper、普通输出/缓存与 P3 ordinary registry 明确拒绝；P5 必须先定义独立番号/站点身份与显式成人工作流再扩展，不能通过普通 Movie 路径绕过隔离。音乐标签写入和 Move/Copy/Hardlink 整理仍未实现，不在本阶段冒充交付。
