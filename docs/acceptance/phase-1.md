# Phase 1 实施与验收记录

- 日期：2026-11-03（Asia/Shanghai）。
- 状态：**P1 实现及本地验收通过；远端跨平台 CI 结果尚未确认**。
- 提交：本记录随附的 Phase 1 提交。
- 用户授权：本轮明确要求通过 subagent 开始 P1；保留 P0 平台启动待验收事实，记录为阶段顺序例外，不回填 P0 成功。
- 停止点：提交并推送后停止，等待用户明确指示开始 P2。

## 完成任务及文件

| 任务 | 交付 |
| --- | --- |
| P1-01 公共实体和身份 | `crates/domain/src/library.rs`：13 种 MediaKind、LibraryRoot、MediaItem、MediaFile、Artwork、ExternalId、Person、PersonAlias、Organization、Credit；UUID 和 UTC |
| P1-02 独立类型扩展 | `video.rs`、`adult.rs`、`music.rs`：Movie/TV/Anime/JAV/Music，编号映射、基础视频关联、番号命名空间、音乐发行/碟/曲目/录音身份；只做结构和序列化 |
| P1-03 来源与保护 | `metadata.rs`、`infrastructure/src/library.rs`：来源、语言、置信度、人工/锁定字段持久化；Provider 原子保护，用户模式显式修改 |
| P1-04 SQLite 与 Repository | `infrastructure/migrations/0001_library.sql`、`src/library.rs`：Root/媒体/文件/图片/外部 ID/元数据 CRUD，FK/WAL/busy_timeout、索引和 keyset 分页 |
| P1-05 路径与移除语义 | `domain/src/paths.rs` 和 Repository：显式 Windows/Posix 词法规则、Root+相对路径唯一、ownership 保护；SQL 删除只移除索引 |
| P1-06 任务意图持久化 | `domain/src/tasks.rs`、`infrastructure/src/tasks.rs`、`0002_task_records.sql`：任务/刮削运行/文件操作记录、不可变快照和状态存取；不执行任务 |
| 设计和可运行示例 | `domain-model.md`、`database-schema.md`、`phase-1-contract.md`、`infrastructure/examples/library_roundtrip.rs` |

分工：设计 subagent 先冻结模型/Schema/API；领域 subagent 实现模型和路径；SQLite subagent 实现公共迁移/Repository/集成测试；主代理审查后放行迁移，完成任务存储、示例、工作流、最终集成与验收。

## 实际测试结果

| 检查 | 结果 |
| --- | --- |
| `pnpm check`（pnpm 12.17.0） | Biome、TypeScript、Vitest、Vite 生产构建全部通过；11 项前端测试 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo test --locked` | 默认 core/SQLite 测试通过；最终 31 项，包含临时库集成测试 |
| `cargo test --workspace --locked` | 包含 Tauri 宿主的编译、链接和全部测试通过；31 项，宿主没有新增独立测试 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 完整 workspace、测试及 example 通过 |
| `cargo run -p mediaatlas-infrastructure --example library_roundtrip --locked` | Fixture Root/媒体/文件索引、分页、人工字段保护及数据库重开验证通过 |
| `actionlint -shellcheck= .github/workflows/ci.yml .github/workflows/release.yml` | actionlint 1.7.12 语法、表达式及工作流校验通过；未执行平台打包 |

31 项 Rust 测试由领域 13 项、LibraryRepository 10 项、TaskRepository 3 项及原工程 5 项构成。自动化覆盖：

- 类型序列化、季集编号方案、Anime 关联、JAV 番号命名空间和音乐不同 Release/Medium/Track/Recording。
- Windows 盘符/UNC/ASCII 大小写、Posix 大小写/反斜杠、路径遍历和非法名称拒绝。
- 迁移重开/重复运行、checksum 不一致拒绝、多连接 PRAGMA、唯一约束与外键级联。
- 所有 MediaKind、多 Root/外部 ID、相同时间戳 keyset、Root EXISTS 去重、毫秒时间边界、搜索字面转义。
- 人工/锁定字段、不同语言独立持久化、等价 locale 不能绕过保护、子记录不能重绑其他作品。
- 任务快照不可变、状态更新与重开、缺失父记录拒绝、审计关系的 CASCADE/SET NULL。
- 真正临时源文件/图片在索引移除后内容不变；文件操作记录不执行计划。

当前执行平台为 Debian Linux x86_64；Rust 1.90.0、Node v24.19.0。完整桌面 workspace 检查沿用 P0 的临时 GTK/WebKit sysroot，默认 core/SQLite 测试不依赖桌面系统库。工具和缓存位于临时目录，不修改用户 HOME。

## 审查发现与修复

1. 迁移前审查确认 Root 通过文件关联作品、外部 ID 不跨作品全局唯一、字段更新采用 SQL 原子保护，具体记录见 `phase-1-contract.md`。
2. 独立审查发现 `en-US`、`en-us` 和含空格 locale 会形成不同键，可能绕过人工保护。现统一 ASCII 小写键，拒绝歧义空格/空段/非法字符，并新增数据库回归测试；field_key 拒绝首尾空格/NUL。
3. 补充独立 PersonAlias 身份，通过 person_id 关联人员；相同显示别名不会自动合并真人。

## 用户要求的工作流调整

- main push、PR 和手动 CI：只在 Windows/macOS/Linux 执行前端和 Rust core/数据库测试；不构建桌面、不安装 GTK、不上传可执行文件。
- tag push：单独编译 Windows NSIS、macOS DMG、Linux AppImage/deb。三平台全部成功后才创建/更新对应 GitHub Release 和上传包；普通分支 push、PR、workflow_dispatch 均不能触发这条发布流程。
- Release job 才有 `contents: write`；构建与测试只读。只配置流程，本轮没有创建/推送 tag、安装包或 GitHub Release。
- 发布包版本仍来自项目版本文件，tag 不自动改版本。签名、自动更新和完整发布验收留待 P8。

## 范围、偏差和待验证项

- 类型扩展只完成结构及序列化，公共 Repository 可保存不同 MediaKind；不宣称全部类型专属字段已入库。Person/Organization/Credit/类型扩展表按真实业务需求追加迁移，避免空表；领域与身份边界已固定。
- 不新增 Tauri 命令或 UI。数据库由调用方显式创建，桌面启动不自动打开媒体库；示例明确标记 Fixture、只使用临时目录。
- 任务状态机、调度、取消和重启恢复属于 P2；元数据跨 Provider 合并和 Preview 属于 P4；P1 不执行文件写入、移动或标签修改。
- 路径规范化为词法身份规则，尚不覆盖符号链接、Windows 全部 Unicode/短文件名语义；locale 不声称完整注册表/语言别名解析。
- JSON 快照递归拒绝常见凭据键，但调用方仍需提供脱敏快照和凭据引用，不能宣称可识别任意文本中的秘密。
- Windows/macOS CI 运行结果、P0 桌面运行待办以及 tag 安装包生成均未在当前环境验证。GitHub API 在该环境访问受限，不能将本地静态检查等同于远端 CI 成功。

下一阶段仅列清单，不开始执行：P2 Root 管理、手动/增量扫描、Parser/只读标签、本地候选预览入库、持久化任务运行、真实媒体库 UI。必须等用户明确指示后再启动。
