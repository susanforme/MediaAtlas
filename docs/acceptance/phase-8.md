# Phase 8 验收：维护与 beta 分发

状态：本地实现及独立维护安全审查已通过；实际 Release 构建和下载验证待执行。本文不以工作流存在、tag 或本地编译替代发布完成。候选版本 `0.1.0-beta.1`；阶段契约见 [phase-8-contract.md](../phase-8-contract.md)，升级和故障恢复见 [maintenance.md](../maintenance.md)，原生构建矩阵见 [release-matrix.md](../release-matrix.md)。

## 交付范围

| ID | 实现及证据边界 |
| --- | --- |
| P8-01 | Windows x64 NSIS 与便携 ZIP；实际 executable PE 架构/版本和全部许可资源提取验证；交互式安装/升级/卸载保留 pending |
| P8-02 | macOS x64/ARM64 DMG 与 Linux x64 deb；4 个原生 runner 并行，Windows/Linux ARM 本次 `not_built`，AppImage 因捆绑原生库对应源码分发门槛尚未完成而 `not_built`；无发行签名/Apple 公证，不能宣称未执行的安装或实机播放 |
| P8-03 | 手动下载核验和退出更新流程、稳定 app 标识/数据目录、SQLx 向前迁移/future migration 拒绝、候选库迁移验证、安全快照及保全故障材料 |
| P8-04 | 可操作维护 UI，完整备份 Preview/Confirm，SQLite `VACUUM INTO`、manifest/schema/app/checksum 与完整性验证；下次启动在任何 pool 前取得终身 OS 实例锁，当前库安全快照、恢复 journal 与无覆盖隔离；safe allowlist diagnostics，普通缓存重置隔离 |
| P8-05 | 复用有效性能回归，真实依赖审计与完整锁文件许可清单；FFmpeg/ffprobe 只用于开发夹具，不随包分发 |
| P8-06 | 版本/资源/架构/许可预检、实际包拆检、SHA256SUMS/manifest/notes；完整上传 draft 后才公开 beta；所有未验收项明确保留 |

完整备份可能包含成人元数据，因此存在隐私数据时必须当前解锁且旧 grant 不能复活；普通诊断不包含私有标题、路径、URL、原始错误、payload 或 token。备份不读取源媒体但可能含 DB 已有的历史音乐标签恢复字节，不含 keyring/session 秘密；跨系统恢复的 opaque ref 不代表已配置认证。源媒体不受维护操作影响，恢复后不重放媒体写入；缓存重置仅移出活跃目录并保留隔离字节，不宣称释放空间。

## 本地验证

最终维护发布竞态修复版完整检查：

| 检查 | 实际结果 |
| --- | --- |
| `pnpm check` | 通过：Biome、TypeScript、156 tests / 25 files、生产构建；主 bundle 485.75 kB（界面精简后） |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过：完整 workspace/Tauri/测试，0 warnings |
| `cargo test --workspace --locked` | 通过：303 项，0 失败（210 infrastructure unit）；不计 0 项 doc-tests |
| 维护独立复核 | 25 项维护基线独立通过，另 14 项兼容/备份/manifest 专项独立通过；最终完整 suite 含 27 项维护与 12 项迁移兼容，安全 review 无剩余阻塞 |
| 许可清单 | 971 dependencies / 1,587 notice references；锁文件、完整文本与 9 原源包 SHA 验证通过 |

维护 27 项覆盖提交后的 WAL 内容、成人授权/epoch 撤销、预览后新私有数据、checksum/未来版本/迁移/FK 损坏拒绝、待启动恢复当前安全快照和源媒体保全、单实例锁、外部同名碰撞与中断幂等、无秘密的安全诊断、缓存同 mtime/大小下换字节/祖先置换、锁后撤销私有待恢复计划、跨系统凭据缺失，以及 restore 候选、回退原库和 backup 实际发布对象/symlink/manifest 替换。实际 32 MiB SQLite VACUUM 运行中取消后原库完整性与原内容保全。原库、candidate、未完成备份和缓存隔离均保留，无自动 unlink。

SQLite VM progress handler 每 4,096 指令检查取消、120 秒预算与旧 grant；数据库逻辑页 1 GiB 上限，hash 120 秒。最多 32 个十分钟单次预览，最多 128 份列表备份；缓存仅普通根直下允许图片、20,000 entries/512 MiB。Unix 上任意同 UID 恶意进程仍可改动 SQLite 绝对路径；维护通过持有 ancestor capability、前后 identity/hash 与实际发布对象复核失败保全，不能宣称 OS 对抗隔离。

跨平台 fixture 修复独立执行：`cargo test -p mediaatlas-infrastructure --lib --test browse --test library_ready --test music_library --test scan_service --locked --offline`（允许 localhost Mock）218 项通过。临时目录在 canonical 系统临时父目录创建，Windows extended/UNC 前缀与本机 PathStyle 遵循正式 Root 注册语义；不放松生产 descendant no-follow 或预览后祖先置换拒绝。Windows `cap_fs_ext` MetadataExt、cap_std creation_time、atomicwrites no-replace 及 macOS rustix API 实读依赖源码核对，仍不替代原生 CI。

既有性能证据复用并由完整 Rust suite 覆盖：`infrastructure/tests/browse.rs` 1000 条游标分页无遗漏/重复；`media-scanner/tests/scanner.rs` 10,000 文件扫描与取消；Provider 故障隔离/速率/cancellation 测试及图片 hash 去重与有界处理。不是跨平台性能 SLA 或真实用户库基准。

依赖审计区分实际编译图与 lockfile：Vitest 开发依赖链已升级至 4.1.12，实际 `pnpm audit` 0 findings。RustSec 中 SQLx 未启用适配器的 rsa 锁文件残留无实际 compile graph 节点；Linux GTK/Tauri glib 0.18.5 的 `RUSTSEC-2024-0429` 及维护告警保留，具体 reachability、版本和原始结果见 `scripts/licenses/security-audit.json`。不能将这些结果写为无 Rust 安全告警。项目源码许可未指定，不添加授权；完整第三方原文及必要源包随发行资源提供。

## 平台和发布记录

实际 CI run、包清单、下载/hash/架构复验结果将在运行后记录。当前 GitHub API runtime 网络策略尚未包含 `api.github.com`，已保存的新配置不等于 runtime 已发布应用；这属于网络出口阻塞，不据此声称缺少 GitHub token。Git HTTPS main 推送正常。不得通过改 host、直连或关闭 TLS 校验绕过网络策略。

真实 Windows/macOS 安装/更新/卸载/默认应用打开、系统 keyring、真实 Provider 账号与可信代理仍 pending。Linux 真实标准发行环境构建、WebKit native 启动/截图按实际执行记录；浏览器 Mock UI 截图明确标注。媒体库没有内置播放器/FFmpeg runtime。测试不操作用户真实媒体。

用户要求的应用截图已在界面精简后全部重拍：[screenshots](../../screenshots/README.md) 含 5 张中文浏览器 Synthetic Demo/Mock IPC 与 5 张英文 Linux 真原生截图，逐张实际查看，非空、无 loading/error、无秘密输入。候选应用源码为 `d50b4dd03c15e45428c4b455f3e8755d40732a48`，版本 0.1.0-beta.1；原生二进制 SHA-256 为 `57160fbd60084cc6f5e6c1b74032bbd151c31db28eb6b42eb2b55dc1140deac1`，标准关窗 app/DBus session 均退出 0。复用合成数据，仅导航取图；没有重新执行扫描、备份或安装验收。截图输出使工作树非空，但实际核对应用源码输入与候选 A 完全一致。

Linux 本地候选实际验证：标准 Ubuntu 22.04 内使用自身 `/usr` WebKitGTK/GTK/DBus 与 glibc 2.35，不继承宿主 sysroot 环境；`.deb` 16,095,924 bytes，包内 0.1.0-beta.1/amd64，ELF x86_64、最大 GLIBC 符号需求 2.34，动态依赖系统 WebKitGTK 4.1/GTK3/libdbus。拆包没有其他 ELF/so 或浏览器引擎/FFmpeg，全部许可逐文件 SHA 校验通过。真实 dpkg 安装后，非 root 的 Tauri/WebKitGTK+Xvfb 运行、合成 NFO/临时媒体确认、备份与标准 WM_DELETE_WINDOW 退出 0 均执行；同版本重装及卸载后，专用 synthetic XDG 数据目录 15 文件的 SHA 全保持。跨版本升级未执行，不能把重装写成升级。

这些证据来自明确标识的未提交本地候选，便于验证打包流程；不代表最终 source commit 的正式构建。正式 Release 仅由最终 clean tag 在四个原生 CI runner 重新构建、拆包和校验，不通过给旧候选 manifest 改 SHA 代替。历史宿主 `/usr` helper 缺失已由标准容器解决，不列为正式 deb 的依赖问题。真实 OS keyring、实时 Provider、Windows/macOS 交互式安装/升级/卸载仍 pending。

LF/CRLF 专项兼容仅允许官方精确 SHA384、完整成功前缀及官方实际 schema；事前一致内部快照与事务 CAS 保全，不接受额外索引/触发器或未知历史，不修改原备份。由独立 reviewer 针对 history 表、快照置换、manifest 与候选绑定反例复核；不泛化为任意 checksum 修复。

界面精简与规则已提交为候选 A `d50b4dd`：删除重复产品介绍/次级标签，保留字段名称、反馈、真实元数据及影响决策的备份/恢复/缓存/会话说明；根 `AGENTS.md` 固定六条规则。完整 `pnpm check` 仍为 156 tests / 25 files、Biome/typecheck/build 通过。P8 source `6db616a` 的 main run [41615154922](https://github.com/susanforme/MediaAtlas/actions/runs/41615154922) 已获得 Windows/macOS/Linux 全部成功；最终截图提交后的新 main CI 与 clean tag 原生分发仍须实际执行。
