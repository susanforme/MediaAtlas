# Phase 6 Music 实施与验收记录

- 日期：2026-11-03（UTC）。基线：P5 `b2701c3`，继承用户新增 Library 消费边界文档提交 `11ea4c0`。先冻结 [P6 契约](../phase-6-contract.md)，再按实体/Provider/标签/UI 文件所有权分工。
- 状态：独立 Music 实现与本地自动化验收通过；实际结果如下。实时 Provider 与原生系统关联应用、Windows/macOS 运行仍 pending；不代表全平台 MVP/发布验收。
- 用户授权连续阶段开发；本阶段只在临时 SQLite、合成短音频与合成图片上测试，未扫描/改写用户真实媒体。未创建 tag 或发布应用。

## P6 逐项结果

| ID | 实现与证据 | 边界/未验收 |
| --- | --- | --- |
| P6-01 | 独立 Artist/ReleaseGroup/Release/Medium/Track/Recording 表、Artist credits/aliases、MBID/ISRC/UPC/EAN/Catalog Number、Track→本地文件关系；原子导入，具体 Release MBID 稳定身份，Track 不等于 Recording | 一份本地音频只关联一个当前发行版本的 Track，跨 Track 重绑冲突；不自动合并同名专辑 |
| P6-02 | MusicBrainz 真实 release 搜索、详情及有界 artist/aliases API；CAA 真实封面 metadata/bytes；全走 P3 NetworkEngine、Provider 设置/任务/审计/取消 | 公共 API 无需 secret；Discogs 未接入，按访问条件保持 optional；live 未配置/未调用 |
| P6-03 | 具体 Release ID、Artist/Album、日期精度、国家、版本描述、多碟与有序 Tracklist 独立匹配；同名同 group 不授权合并；所有分数均保留人工版本选择 | `YYYY`、`YYYY-MM`、完整日期原文保留，不把 partial date parse 失败抹成年份缺失 |
| P6-04 | lofty 真实格式读取、线上选定 Track 字段→本地已知标签选择 Diff；未选字段与封面保留；格式测试矩阵见下 | 支持以已验证格式/字段为界，不能推断所有可能的标签变体都无损支持 |
| P6-05 | 独立只读 Music Library / Artist / Album / Disc Tracklist、来源/语言/受保护字段、后端缓存封面；`/music/tools` 独立搜索/导入/关联/编辑/恢复 | 真实 Chromium 的 `/music` 与 `/music/tools` 中英 route 冒烟通过、0 pageerror（主代理脚本 `phase6-browser-routes.cjs`）；Mock IPC/unit 不等于原生 WebView/IPC 验收；旧影视/成人详情区的操作分离留在后续计划 |
| P6-06 | Preview→明确执行→原字节备份→quarantine 实际来源复核→no-clobber 发布→持久记录；恢复自身也 Preview→明确确认，保留被替换后的字节；取消/过期/重启需新预览 | 256 MiB 单文件全字节快照上限；大文件流式恢复日志未实现；备份不自动清理 |

## 最终检查

| 检查 | 实际结果 |
| --- | --- |
| `pnpm check` | 通过：Biome 72 files、TypeScript、132 tests / 20 files、生产构建；主 bundle 490.16 kB，无体积警告 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过：完整 workspace/desktop/测试，0 warnings |
| `cargo test --workspace --locked` | 通过：206 项功能测试、0 失败；infrastructure unit 129 项，不将 0 项 doc-test 计作功能测试 |

本次最终修复后的全部检查通过。中间新增 edition 匹配回归曾发现遗漏版本描述比较，已补实现并重新跑完整 Rust 检查，不跳过失败测试。

独立图谱临时 SQLite 9 项、系统打开 stub 6 项、Music Provider/transport 真实 Mock HTTP 6 项、封面离线持久读 3 项、标签 10 项、匹配 4 项以及 Music UI/API 16 项覆盖新流程。封面选择在工具区获取/保存，Library 无新 Provider 请求，重建服务后离线读取；缓存原图 hash/JSON 损坏与当前隔离/MBID/取消/锁定都拒绝。

## 音频格式实际测试矩阵

六个 checked-in fixtures 是 FFmpeg 从 `anullsrc` 生成的 0.08 秒静音单声道音频，CC0；测试本身不要求安装 FFmpeg。主代理以 ffprobe 验证 MP3/mp3、FLAC/flac、M4A/aac、OGG/vorbis、OPUS/opus（48 kHz）、WAV/pcm_s16le，其余 44.1 kHz。ffprobe 构造有效性不替代 lofty 标签写入验证。

| 格式 | 实际 lofty 读取/写入 | 未选未知字段/封面 | 原字节备份/恢复 | 合成样本音频验证 |
| --- | --- | --- | --- | --- |
| MP3 | 通过：实际服务修改 title 后重读 | 通过；另测 native opaque binary ID3 frame | 通过 | codec essence + 解码 PCM SHA-256 一致 |
| FLAC | 通过：实际服务修改 title 后重读 | 通过 | 通过 | codec essence + 解码 PCM SHA-256 一致 |
| M4A/AAC | 通过：实际服务修改 title 后重读 | 通过 native atom unknown 字段 | 通过 | mdat essence + 解码 PCM SHA-256 一致 |
| OGG/Vorbis | 通过：实际服务修改 title 后重读 | 通过 | 通过 | 音频 packets + 解码 PCM SHA-256 一致 |
| OPUS | 通过：实际服务修改 title 后重读 | 通过 | 通过 | 音频 packets + 解码 PCM SHA-256 一致 |
| WAV/PCM | 通过：实际服务修改 title 后重读 | 通过 native ID3 unknown 字段 | 通过 | data essence + 解码 PCM SHA-256 一致 |

10 项标签自动化测试覆盖六格式矩阵、native opaque ID3、未知字段/封面、原字节恢复与幂等、取消/discard/重启、不可变 payload、成人重分类、字段保护、源/祖先 symlink、移动前替换/发布前外部新文件、损坏备份与恢复前/后外部编辑。fixture 和生成器位于 `crates/infrastructure/tests/fixtures/music/`。主代理独立将实际 `MusicTagService` 写后 `/tmp/mediaatlas-p6-tagged/` 的六种输出与原 fixtures 使用 FFmpeg `-f hash -hash sha256` 解码 PCM 比较，六种均实际退出 0 且完全一致；本次会话证据 `/tmp/mediaatlas-p6-audio-verification.json`。这只证明这些合成编码样本，不泛化所有用户媒体/损坏文件或所有标签变体；FFmpeg 非运行时/常规 CI 依赖。

## 安全与许可边界

所有普通 Music 读取、导入、链接、标签读写/恢复及原生打开都拒绝成人隔离条目、成人 Root 和重新分类后的旧关联。确认执行持有 SQLite 写锁并重新验证完整媒体/Root/本地文件/关联图谱，防事务前核验后的数据库关联变化。文件能力逐级 no-follow，计划与字节不可变，源 hash/大小/修改时间复核；Unix 验证当前路径与预览目录能力，Windows 保留无删除共享的目录/源文件 lease，同会话释放自身 source lease 进行 quarantine，再验证实际移入的字节。Windows 完整 SHA-256、大小/时间与安全句柄信号是内容保护，不宣称 ReFS 128-bit File ID 已验证。无平台实机运行记录，Windows/macOS 均 pending。

MusicBrainz 项目 User-Agent 使用 `https://github.com/susanforme/MediaAtlas`，NetworkEngine 强制同站不少于 1 秒间隔，通用设置 `interval_ms=0` 不能绕过；尊重 Retry-After。CAA 每个重定向验证 HTTPS、公开 IP、受控编号 archive host 与绑定同一 Release/图片的资源路径；不把 `*.archive.org` 网络允许列表当通用下载代理。图片复用现有全局两个 decoder permit、压缩字节/MIME/尺寸/像素/分配上限、hash 去重缓存。前端只收本地 data thumbnail，不能直连图片 Provider。

MusicBrainz 核心 metadata 为 CC0，补充 metadata 可能 CC BY-SA；CAA 图片版权归具体权利人，保留来源和权利提示，不声称图片自动取得 CC0 许可。真实网络 draft 未发布不等于已允许 live 访问，未知代理仍 fail closed，不绕环境代理/TLS/SSRF。

系统打开只由显式用户点击调用，后端从 `file_id` 解析已授权音频；不接受任意命令、可执行文件、自定义参数或外部 URL。不内置播放器/转码/server。单元测试注入 opener stub，响应只确认已请求系统打开，不声称默认应用已开始播放；实际 OS 默认关联、平台 GUI 与之后外部进程改变路径后的行为未验收。

## 后续

后端 `execute_cancellable` 的在途取消和 UI 的 Preview discard 已测试；音频确认执行没有接 IPC/UI 在途取消按钮，不宣称该交互已实现。Provider 任务仍复用 P3 Task Center 取消。

P7 继续独立整理工作区、文件操作计划/恢复和 Watcher；P8 才处理发布与跨平台安装验收。P5 视频 MVP 的 live/平台 pending 原样保留；下一阶段授权不被解释为缺失平台测试已通过。
