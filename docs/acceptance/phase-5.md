# Phase 5 实施与视频 MVP 验收记录

- 日期：2026-11-03（UTC）；基线：main `e9b6a13`（P4）。先冻结 [P5 契约](../phase-5-contract.md)，再按文件所有权分工，主集成负责成人隐私权限、普通出口隔离、IPC 与验收。
- 状态：独立 AdultVideo 实现已落地；最终自动化检查结果见下表。实时 Provider、真实系统密钥服务及 Windows/macOS 原生运行仍未验收。视频 MVP 尚未满足所有平台门槛，不是可发布版本。
- 本会话授权连续开发阶段并逐阶段 Codex 提交/推送；主代理审查后推送。未创建 tag/Release。开发与测试只操作临时合成媒体、临时 SQLite 和中性合成图像，未读写用户真实媒体。

## P5 逐项结果

| ID | 实现与实际证据 | 限制/未验收 |
| --- | --- | --- |
| P5-01 | 独立 CatalogNumber 保留 raw/normalized/namespace/primary/aliases 与 CD/Part；大小写/连字符/下划线/零填充、FC2-PPV 和数字前缀表驱动测试；冲突候选显示 review 原因，不沿用 Movie 评分 | CD/Part 保留文件分段，不据其合并不同发行商作品 |
| P5-02 | `dmm` 官方 FANZA ItemList API 真实请求代码，走 P3 NetworkEngine；搜索→详情→图片候选→确认入库；Mock 实际 HTTP 验证双字段 query auth、源 URL 净化和错误详情拒绝 | approved API ID + affiliate ID、条款/图片复用权、实时授权/robots 当前状态与网络访问 pending；默认 disabled |
| P5-03 | `wikidata` 第二独立真实 SPARQL 适配，CC0 公共结构化补充；强制成人作品类型、P528 statement 的 P972 catalog namespace、P123 publisher；实际 Mock 与 DMM 聚合、403 隔离、单站禁用均通过 | 不是广覆盖 JAV 站点。公共 P528 单值不能识别成人作品；缺类型/issuer/catalog 上下文时拒绝。多站 live 可用性未验收，不能据 Mock 标为多站 live 成功 |
| P5-04 | 详情复核番号/namespace/remote ID/发行商 stable ID，DMM 同名不同 maker.id 实际 Mock 明确 IdentityMismatch；重发/namespace 冲突 review；人员保留 Provider-scoped ID 与 aliases，不按同名自动合并 | 无人工跨站真人别名映射管理器；保留来源实体，不推断真人身份 |
| P5-05 | 独立成人工作台与详情，番号优先查询、站点健康/启停、Studio/Label/Performer/Series、所选封面/样品图、来源字段 Diff、Preview/Confirm；Rust typed IPC 到 SQLite 完整闭环；zh-CN/en-US UI 测试 | 原生 WebView/IPC 实机未验证；UI unit/Mock IPC 不替代 native 验收 |
| P5-06 | 持久“始终显示成人媒体入口”偏好，和 unlock/covers/recent 分开；默认隐藏/封面关闭/最近访问关闭，重启不恢复解锁。后端 revocable grant、隐藏/锁定旧请求拒绝、独立 Root/search/cache；普通统计/墙/详情/Tasks/Providers/Inspector 始终排除成人 | 本地隐私控制不是 OS 认证边界；Linux rootless GUI helper 问题仍在 |
| P5-07 | 独立有类型成人输出通道、计划持久 channel/current Root/source 身份复核；NFO/cover/back-cover/sample 及有界 batch 均 Preview→显式确认→执行→记录，默认 Preserve。全局 batch cancellation 支持不同 IPC 实例，unknown/普通任务 UUID 取消不变更其它 lane；失败/重启只记录，不自动重放 | Windows no-delete lease adapter 未实机运行；硬链接不支持时 fail closed，不降级覆盖 |
| P5-08 | 以下十项基线视频 MVP 逐项列证据，保留跨平台与 live pending | 当前不能宣称视频 MVP 已全量通过或正式可发布 |

## 最终检查

| 检查 | 结果 |
| --- | --- |
| `pnpm check` | 通过：Biome 66 files、TypeScript、116 tests/18 files、生产构建；主 bundle 481.12 kB，无体积警告 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过：完整 workspace/宿主/测试，0 warnings |
| `cargo test --workspace --locked` | 通过：168 项测试，0 失败；其中 infrastructure unit 104 项，不把 0 项 doc-test 计作功能测试 |

主代理独立 review 最终通过。真实 Chromium + 明确 Mock IPC 的持久入口冒烟通过：backend 返回 entry_visible=true/unlocked=false 时侧栏入口存在，直接成人 route 不发 private reads；unlock→lock 后入口仍在，reload 后仍显示但内容锁定，0 pageerror。脚本 `/workspace/.mediaatlas-tools/phase5-entry-browser.cjs`；这只证明浏览器 Mock UI 行为，不代表实际 DB 重启/原生 IPC。

Mock HTTP 需获准本地 loopback 监听；未请求真实成人内容或真实封面。测试涵盖多个 query secret 的 redirect/反射拒绝、认证 revision 轮换、成人 typed search/detail/merge、字段锁定/同毫秒 canonical edit、root relocate/disable、图片 await 期间取消/lock 无写库、普通快照无成人、入口/锁/最近访问并发、独立 batch 实例在途取消、原视频字节不变、选择性 NFO 保留和 capability/no-follow/no-clobber 输出。

## 视频 MVP：基线第 14 节逐项

| # | 基线条目 | 真实证据与状态 |
| --- | --- | --- |
| 1 | Windows 桌面应用可运行 | **Pending**：本次 Linux 环境不能验证 Windows 启动，P0/P4 平台未验收项保留；未声称满足 |
| 2 | 多个本地 Root | P2 Repository/Browse/Root UI 集成测试；P5 增加成人专属 Root 及普通/成人目录重叠拒绝；本地自动化通过不等于 Windows E2E |
| 3 | 扫描本地视频并建立媒体库 | P2 Scanner/Import 临时 Fixture 集成及重复扫描/取消/恢复测试保留；P5 成人 workspace 提供独立 Root、扫描进度、候选预览与确认入口 |
| 4 | 真实海报墙/List/搜索/筛选/详情 | P2 1000 条分页与过滤、本地缩略图测试；P5 普通查询始终普通 lane、成人独立详情/图片与 UI 测试 |
| 5 | TMDB API + HTML 示例 | P4 TMDB Movie/TV 与 Wikipedia 单篇 HTML 真实适配、P3 Mock 契约测试；**Live pending**，缺本实例有效凭据/可信网络授权，不把 Mock 当成功 |
| 6 | JAV 番号、至少一个 Provider、独立页面 | P5 DMM 真实 API 接入 + 独立AdultQuery/Candidate/Detail、番号规则、真实Mock闭环与独立工作台；**Live pending**，第二源有限覆盖/未验证多站 live |
| 7 | 任务队列/进度/失败/取消 | P2/P3 持久任务和中断恢复；P5 Provider failure isolation、独立成人 scans/providers、batch逐项结果、取消不跨 lane；UI 与 Rust 测试 |
| 8 | 字段来源/用户锁定/Merge Preview | P1/P4 保护规则；P5 独立 adult merge immutable snapshots、事务 CAS/current media/root/grant 复核、锁定/人工字段保持 |
| 9 | NFO/海报导出，默认不移动源视频 | P4/P5临时文件 Preview/确认/输出/重复执行/冲突/取消/源字节不变。成人通道不放宽普通拒绝边界，既有sidecar仍 Preserve |
| 10 | Parser/Provider/Matching/Repository/文件Preview自动化 | 完整 workspace Rust + pnpm check，最终数量见上表；fixture数据明确 synthetic，不记录虚构 Provider 成功率 |

TV 与基础 Anime 的 P4 验收仍有效，不因单个 Movie/JAV 闭环省略。当前是视频 MVP 候选实现，本地自动化门槛、实时授权和原生平台门槛分开记录。继续 P6 时保留上述 pending，不把后续开发授权解释成验收成功。

## 认证、安全与遗留边界

DMM Settings 两个 masked 字段构造成 `affiliate` JSON，仅存 OS keyring（默认）或用户明确选择的 SessionOnly；SQLite/Job 只有随机 revision-specific opaque ref。API ID/affiliate ID 均通过 P3 `secret_queries` 注入真实请求，从 Inspector/source URL/snapshots 排除；跨源 query-auth redirects fail closed。Settings 状态不回秘密。通用 Provider limits 不允许改 credential_ref，用户认证只通过 Settings，已删除 raw-ref 产品输入框。

环境当前已观察的 policy 仅 package-manager hosts，runtime secret/bindings 为空。主代理保存的新增 domain draft 未发布不等于已授权可访问；未绕过代理、TLS、SSRF、robots、登录/付费/验证码。详见 [Provider 访问条件](../provider-access.md)。

Windows/macOS 启动、真实 OS keyring 写删、真实 live 权限、远端 CI、安装/升级/签名和发布均 pending。P6 音乐仍须独立 Release 身份、标签 Diff/备份/恢复；P7 Move/Copy/Hardlink 整理及 P8 发布工作留待各阶段，不在此冒充实现。
