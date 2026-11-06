# 发布矩阵与验证

2026-11-06 准备发布 `v0.1.0-beta.12`：按用户确认新建 beta.12，保留已公开 beta.11。媒体工具依据 Primer、Carbon、tinyMediaManager 等公开实践调整二级导航、资源列表、选择批次、详情聚焦及监控状态；研究依据与限制见 [媒体工具 UI/UX 调整](media-tools-uiux.md)。普通导航状态独立于后端筛选，核对会话、成人隔离、显式预览确认和只读源文件边界保留；此前 NFO 独立窗口与包体优化全部沿用。版本及 Cargo.lock 指纹同步更新，第三方依赖不变。本地不运行项目、脚本、测试、格式化、构建或安装；云端先通过三平台类型检查和测试，再创建新标签走四目标发布与完整资产校验。实际布局与系统交互未实机验收。

2026-11-06 准备发布 `v0.1.0-beta.11`，包含 `15c21a5` 的发布包未授权 Tauri 命令裁剪和 `b2bce3f` 的 NFO 独立窗口、共用影视库详情与标签页、受限图片预览及 JSON/侧栏 NFO 入口移除。沿用 beta.10 的符号剥离、thin LTO、LZMA、ZIP 级别 9 与包内许可材料排除策略。所有本项目版本及 Cargo.lock 指纹同步更新，第三方依赖不变。遵守用户约束，不执行本地项目、脚本、测试、格式化、构建或安装；新标签仅在云端四目标构建和完整资产校验通过后公开。系统双击、多窗口及视觉效果仍未实机验收，实际包体变化以新发布清单为准。

此前 beta.10：NFO 与媒体库详情复用纯展示组件，缺失资料区域省略；包内许可材料移至 Release 附件，Rust 发布编译、NSIS 与 ZIP 同步优化体积。版本清单及锁文件统一为 beta.10，许可清单锁文件指纹同步更新。

2026-11-05 当前安装包策略：所有平台的安装资源与可执行文件均不再包含许可材料，也不提供运行时许可导出命令。完整 `LICENSE.md`、`THIRD_PARTY_NOTICES.md`、`dependency-licenses.json`、`license-materials.zip` 继续作为 Release 附件分发并校验 SHA-256。Windows NSIS 改用 Tauri 默认的 LZMA 压缩，便携 ZIP 与许可附件 ZIP 使用标准 Deflate 级别 9。实际瘦身幅度以新标签的四平台云端产物为准；本轮未运行本地构建、测试、脚本或安装包。

Rust release 配置使用 `opt-level="s"`、thin LTO、单 codegen unit 和符号剥离。编译/链接可能更慢，原生回溯可能失去函数符号；panic 仍保持默认展开，错误清理路径继续保留。不能把配置变化当成已实测的体积下降，实际包大小由新标签云端产物确认。

发布标签支持 `vX.Y.Z` 正式版和 `vX.Y.Z-beta.N` 预发布版（N 为正整数，所有数字段不含前导零）。workflow 接收 `v*`，预检拒绝其他格式，继续要求包清单、Tauri、Cargo workspace 与本项目锁文件版本完全一致。正式版设为 `prerelease=false`，通过 GitHub `make_latest=legacy` 按发布时间和语义版本决定 latest；beta 保持 `prerelease=true`、`make_latest=false`。四平台、来源绑定、草稿上传及资产完整性门槛共用。此脚本扩展不移动正在构建的 beta.9 标签。

2026-11-05：按用户要求准备发布 `v0.1.0-beta.9`。桌面业务样式全部迁移为 Tailwind 4，删除 9 个组件/feature CSS 文件和启动页内联样式；全局仅保留主题 token 与 Tailwind reset。保留现有测试使用的语义 class/id，更新下拉菜单坐标与主题测试的实现引用，以及 QA 来源文件清单。根与前端 `AGENTS.md` 加入仅使用 Tailwind、保留 UI 的约束。根/desktop 包、Tauri 配置、Cargo workspace 与锁文件的六个本项目 crate 同步为 beta.9；第三方依赖不变。按用户要求仅静态阅读、修改和 diff 核对，未运行本地项目、测试、lint、构建、格式化或安装包，未生成本轮截图，实际视觉一致性未经渲染验证。发布沿用 tag 触发的 GitHub Actions 原生四平台构建与验证，只有远端门槛全部通过才公开 Release。以下 beta.8 及更早版本记录为历史背景。

beta.9 首次云端预检在 `bf0cd14` 因许可清单的 Cargo.lock 指纹仍为旧版本而停止，未调度打包或公开 Release。补齐六个本项目 crate 升版后的锁文件 SHA-256，第三方依赖记录及许可材料不变；保持不运行本地生成器，修正后更新尚未发布的 beta.9 标签重试。`bf0cd14` 的 Windows/macOS/Linux main CI 已全部通过；正式发布结果以修正后标签提交的 Actions 为准。

2026-11-05 本轮按用户要求移动现有 `v0.1.0-beta.8` tag 并 push 重新发布，版本号和第三方依赖不变。新增 IMDb 完整地址／ID 精确识别、人工身份纠错预览，以及 OMDb、豆瓣、Bangumi 三个来源；来源配置、目录优先级和 Kodi 图片导出同步接通。仅静态修改并补写回归，未执行本地测试、lint、构建、格式化或安装包；远程发布结果以对应提交的 Actions 状态及资产清单为准。下文首次创建 beta.8 的描述为历史背景。

发布前读取 main 上 `97ceba8` 的 Actions：Windows、Linux 成功，macOS 的 12 项 NFO 测试因临时目录含系统符号链接报 `NotADirectory`。相关 fixture 改用仓库现有的 canonical 临时目录 helper；生产目录的 no-follow 边界保持。修复未在本地执行，后续结果以新提交的云端检查为准。

仅推送 `vX.Y.Z-beta.N` tag 触发 `.github/workflows/release.yml`；main/PR 保持前端与 Rust core 测试 CI。本地 `pnpm build:desktop` 使用 `--no-bundle`。本次发布版本为 `v0.1.0-beta.8`，包含媒体库交互与来源配置流程重构，并补齐扫描结果进入来源设置时携带返回媒体库参数的测试断言。按用户要求仅静态阅读和修改，未运行本地测试、lint、构建或安装包；包清单、Tauri 配置、Cargo workspace 及锁文件中的本仓库包版本同步为 beta.8，不变更第三方依赖。新建 beta.8 tag 并推送，beta.1–beta.7 保持原提交不动。NFO 关联与 macOS 固定文案的系统语言边界见 [NFO 预览说明](nfo-preview.md)，TVmaze 来源与归属要求见 [Provider 访问说明](provider-access.md)。

| Rust target | 官方 runner label | 产物 | 发布要求 |
| --- | --- | --- | --- |
| `x86_64-pc-windows-msvc` | `windows-2022` | NSIS `.exe`、便携 `.zip` | 必需 |
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | `.deb`；AppImage 本次不构建 | 必需 |
| `x86_64-apple-darwin` | `macos-15-intel` | `.dmg` | 必需 |
| `aarch64-apple-darwin` | `macos-15` | `.dmg` | 必需 |
| `aarch64-pc-windows-msvc` | `windows-11-arm` | NSIS `.exe`、便携 `.zip` | 本次不构建；后续独立验证 runner/打包兼容性 |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.deb`；AppImage 本次不构建 | 本次不构建；后续独立验证 AppImage 工具链兼容性 |

此处 x86 指成熟的 x86_64/x64，不提供 32 位 x86。四个必需目标全部通过才能发布。两个额外 Windows/Linux ARM 目标仅保留为未来候选，本次不调度 runner，发布说明与 `release-manifest.json` 明确记录 `not_built` 和原因；macOS ARM 已提供成熟 ARM 组合。这样尚未可用、可能长期排队的可选 runner 不会阻挡必需目标发布，也避免发布时抢先下载造成迟到包与校验和不一致。矩阵以 `scripts/release/targets.json` 中 `required: true` 为唯一运行时来源；将来扩展必须实际通过全部校验。

## Runner 与构建证据边界

2026-11-03 查询的 [GitHub 官方 runner-images 列表](https://github.com/actions/runner-images#available-images) 明确列出上述 label 和架构；没有采用 deprecated 的 macOS 14，也没有采用付费 `-large`/`-xlarge`。使用同架构原生 runner，而非在一个 runner 顺序交叉编译全部平台；不使用浮动 `*-latest`。

每个目标独立 job，`fail-fast: false`、`max-parallel: 4`。每 job 限制 `CARGO_BUILD_JOBS=2` 以降低内存峰值；缓存额外 key 包含 runner label、Rust target、Cargo.lock 和 rust-toolchain.toml 哈希，rust-cache 自身也隔离平台/toolchain。工具链固定为仓库 `rust-toolchain.toml`，显式安装目标 Rust 标准库；Node 24、pnpm 使用项目 `packageManager` 固定版本。

每目标均运行 frontend tests、`cargo test --locked --target <native-target>` 和真实 `tauri build --ci --target ... -- --locked`。Linux 安装 WebKitGTK 4.1 等系统开发依赖，使用 Ubuntu 22.04 以降低 glibc 基线。Windows 使用 MSVC/NSIS，release overlay 显式固定 build.windows.staticVCRuntime=true、bundleVCRuntime=false；实际应用 PE normal/delay imports 必须不依赖 VCRUNTIME/MSVCP 等额外 VC redistributable，便携 ZIP 不以 runner 已装开发工具当运行前提。NSIS 当前显式采用 LZMA；此前 BZip2 解决旧 zlib 的 7-Zip 提取失败，仅为历史背景，见 [Windows hotfix](hotfix-windows-ci.md#nsis-package-extraction-2026-11-03)。macOS 使用原生 Apple 工具链。`tauri build` 负责前端生产编译，完整矩阵和资产验证通过后再集中发布。

上述 label 文档、工作流静态校验以及 Linux 本地 Rust 通过，都不能替代 Windows/macOS 真实编译、启动或安装验收。首次远程 workflow 的 job/产物结果才是各目标构建证据。交互式安装、升级、卸载、用户数据保留、系统 keyring、真实 Provider 访问继续按 P8 平台验收记录；manifest 明确 `installation_tested: false`。

## 产物门槛

`release.py preflight` 严格核对正式版或 beta tag 格式以及根 package.json、desktop package.json、workspace Cargo.toml、Cargo.lock 的所有本项目 crate、Tauri config 的版本一致。任何版本冲突都会阻止发布。

`release.py stage` 首先解析原生可执行文件头（PE/ELF/Mach-O），检查真正的 CPU 架构与非空文件；然后逐包验收：

- NSIS：7-Zip 解开安装器，读取内含应用 exe 的 PE 架构与 Windows ProductVersion。NSIS 启动器自身可能为 x86，因此不以安装器启动器架构误判应用目标。
- 便携 ZIP：成员必须恰好为 `MediaAtlas.exe`，不附带目录或零散文件。验证 ZIP CRC、实际 PE 架构、normal/delay CRT imports 和原始 exe SHA-256，不执行提取出的应用程序。NSIS 安装器保持独立资产。
- Debian：`dpkg-deb` 检查包内完整 version/Architecture，再解开检查实际应用 ELF；拒绝额外 ELF、`.so`/DLL/dylib，确保不夹带 GTK/WebKit helpers/shared libraries，依赖系统 runtime。
- AppImage：本次明确 `not_built`。Tauri AppImage 会带入 GTK/WebKit 等原生库；实际归属、逐项版权及同处对应源码分发 gate 尚未完成，不能以 Rust/npm notices 冒称覆盖。preflight/stage 显式禁止重新启用此格式。保留底层提取/架构检查工具供未来开发使用，不代表可分发。
- DMG：`hdiutil` 只读挂载，检查 app 的 Info.plist 版本及实际 Mach-O；无论检查结果如何都卸载。

所有原生包提取/挂载后检查不含许可清单、归档及许可资源目录；所有应用二进制检查不含已移除的嵌入许可目录标记。聚合拒绝没有记录材料排除检查的目标 manifest。许可全文及对应源码归档只保留在独立的 Release 附件中，按源码字节、完整文件集合与 SHA-256 校验。Apple bundle short version 或 Windows product version 在工具链只支持数字版本时允许 SemVer core；完整发布版本仍由 tag/config/Cargo/manifest 保持一致。产物以完整版本和 Rust target 重命名，避免不同平台重名。

`stage` 默认只记录 package validation，不宣称自动化测试通过；仅在 workflow 前端与 native Rust core 两步成功后传入 `--tests-passed`。聚合拒绝没有真实测试通过记录的包，本地打包 dryrun 不会自动生成可发布的测试声明。

每目标上传经过检查的包及 manifest，记录 tag、commit、版本、OS、CPU、runner、format、大小、SHA-256、签名状态与实际测试边界。聚合 job 再核对所有必需目标、完整格式集合、路径、重复名、commit/version/target 元数据、实际大小、SHA-256；多余文件、空包、被改动的包或缺少必需格式会阻止 Release。

聚合产物包括所有可用安装包、项目许可/说明/清单、保留全部许可材料目录结构的 license-materials.zip、RELEASE_NOTES.md、release-manifest.json、SHA256SUMS。SHA256SUMS 覆盖除自身以外的每个 Release 资产。GitHub 自动提供的源代码档案链接可由 tag 获取，源码 commit 链接也写入说明。校验和验证字节完整性，不代表签名身份。

## 单 EXE 与独立许可附件

Windows 便携 ZIP 解压后仅有 `MediaAtlas.exe`，直接运行仍要求系统已有 WebView2。所有平台不再嵌入许可文本或对应源码归档，应用不再提供许可导出命令。查看完整许可材料直接下载 Release 的 `license-materials.zip`，无需启动应用。无参数正常启动、单个 `.nfo` 文件关联启动和无效参数拒绝仍保留。

`test_portable.py` 验证无资源配置、单文件 ZIP、CRC、原始 EXE 哈希、架构及 CRT 边界，且不会执行被提取的应用。发布验证同时拒绝重新混入许可文件或旧嵌入目录标记，外部许可附件继续逐字节及哈希校验。新增测试仅静态补写，未在本地运行。

历史背景：beta.9 及更早版本曾在 EXE 内嵌和安装资源中同时分发完整材料，并验证运行时导出。此前的 52 项发布工具测试、导出模块内部 3 项 Rust 测试，以及相关旧平台成绩属于旧实现，不能作为当前材料排除策略的验证结果。

## 发布权限与恢复

默认 `contents: read`，仅聚合发布 job 获得 `contents: write`。checkout 禁止保留 git credentials。只有全部必需构建成功且聚合校验通过，才先创建 draft，上传完整资产，最后按标签类型公开正式版或 prerelease；beta 不替换正式版 latest。上传中断会留下未公开 draft。重试复用同标签的 Release ID 并核对资产；遇到意外资产或 immutable Release 时拒绝替换。不要覆盖已发布 tag；修复已公开版本时递增对应版本号。

不启用自动更新、updater artifacts、证书签名或 Apple notarization。未配置凭据时不会伪造签名；macOS 在完整应用资源复制后显式生成 ad-hoc 签名，并在只读挂载的 DMG 内强制验证资源签名完整性；此验证不等同可信分发签名或 Apple 公证，见 [资源签名修复](hotfix-macos-package.md)。Windows SmartScreen、macOS Gatekeeper 可能提示，发布说明明确这一状态。Windows NSIS 显式使用官方 downloadBootstrapper：系统缺少 WebView2 runtime 时在线安装，不捆绑 fixed/offline Chromium/WebView2 engine；ZIP 要求已有 WebView2 runtime。macOS 实际 app 用 otool 验证链接系统 WebKit/WKWebView，禁止内嵌 Chromium/WebKit framework；Debian 包显式依赖系统 WebKitGTK 4.1、GTK3、libdbus-1 与 xdg-utils，实际 ELF 的 DT_NEEDED 必须有 WebKitGTK/GTK/libdbus，readelf symbol versions 的最高 GLIBC 不得超过 2.35；AppImage 本次不分发，缺少其实际原生库归属和对应源码分发 gate；Ubuntu 22.04 构建的 glibc 基线为 2.35，不承诺旧发行版兼容。ffmpeg/ffprobe 仅为开发 fixture 工具，应用不调用或捆绑它们；拆包校验显式拒绝夹带这两个可执行文件。AppImage 候选 overlay 显式关闭 bundleMediaFramework，但此设置不能替代其未完成的原生库分发审查，也不使该格式进入本次矩阵。

项目自身按所有者提供的 MIT License 授权；`LICENSE.md` 原文、第三方许可及对应源码材料完整保留在源码仓库和 Release 附件中。依赖保留其自身许可，维护真实的 THIRD_PARTY_NOTICES.md、docs/dependency-licenses.json 及 docs/third-party-licenses/**，不再将这些材料复制进安装包或应用二进制。

2026-11-04：合入所有者的 MIT 许可提交后，将构建资源、嵌入导出、发布归档和校验入口统一到 `LICENSE.md`，保留逐字节与 SHA-256 校验。许可证 18 项测试、发布 56 项测试通过；4 项原生 macOS 签名测试及 1 项 NSIS 工具测试因当前 Linux 主机缺少对应工具跳过，不作为本轮实机验收。实际便携版嵌入/导出测试使用 Rust 编译运行，未跳过。清单验证为 1002 依赖、1626 notice 引用。

## 本地验证与外部前提

```sh
python3 -m unittest discover -s scripts/release -p 'test_*.py' -v
actionlint .github/workflows/release.yml
python3 scripts/release/release.py preflight --tag v0.1.0-beta.8
```

首次准备时 24 个 verifier 测试与 actionlint 1.7.12 已在本地通过，合并 Tauri release 配置通过官方 JSON schema 验证；preflight 需 P8 的版本统一及真实许可文件齐全。模拟 fixture 测试验证发布校验器的拒绝路径和完整汇总，不把 fixture 当作 MediaAtlas 安装包通过证据。

当前 managed 环境的 enforced HTTP allowlist 缺少 `api.github.com`，代理曾在 CONNECT 阶段返回 403，因此本地 GitHub Actions/Release API 监控受网络策略阻挡；官方 github.com 的公开 Actions/run/job 网页仍可用于只读状态监控，未登录的详细 step 日志端点会返回 404，不能把它解释为没有失败；这不能推断为缺少 GitHub token 或仓库权限。git push 已可用。待用户发布包含 api.github.com 的环境配置且状态实际 enforced 后，再以正常 `gh` 只读命令确认运行结果；不打印凭据、不绕过代理、不反复请求同一个已被策略禁止的地址。GitHub-hosted runners 使用其自己的网络，当前本地 API 监控阻塞并不直接证明远端 workflow 不能运行。

2026-11-03 原 main `8d55797` 的 Windows 失败及后续 macOS fixture 失败已通过独立 hotfix 修复；最终 `1e80669` 的 [真实三平台 CI run 41612577348](https://github.com/susanforme/MediaAtlas/actions/runs/41612577348) 已整体 COMPLETED/SUCCESS（8m36s）。Windows job `125837899288`、Linux job `125837898742`、macOS job `125837899119` 的前端与 Rust core 全部成功；macOS Rust 于 09:45:47 UTC 完成。此证据针对该独立 hotfix，P8 新源码仍需其最终 main CI，再允许创建 beta tag。不得把较早 Linux 本地绿灯当作 Windows/macOS 证据。


## Linux 本地候选联调证据

标准 Ubuntu 22.04 容器以非 root UID 1000 构建过未提交的 P8 本地候选；没有 source 宿主 activate.sh，PKG_CONFIG_SYSROOT_DIR、PKG_CONFIG_PATH、LD_LIBRARY_PATH、LIBRARY_PATH、RUSTFLAGS、CARGO_ENCODED_RUSTFLAGS 均 unset，三个 pkg-config libdir 均来自 `/usr/lib/x86_64-linux-gnu`。managed proxy/只读 CA 保持有效，未关闭 TLS 或 WebKit sandbox。

实际候选 `.deb` 为 16,095,924 字节、amd64、0.1.0-beta.1；真实 ELF DT_NEEDED 包含系统 WebKitGTK 4.1、GTK3、libdbus-1.so.3，最高 GLIBC symbol version 为 2.34，不超过声明的 2.35；拆包架构/非空/版本/许可字节以及没有多余 engine/native ELF 全部通过。实际 dpkg 首次安装成功；真实后端/隔离 AppData 的原生 UI 启动、备份和五页截图由 screenshots 记录。标准 WM_DELETE_WINDOW 正常关闭返回 0；早期 XDestroyWindow 的 133 属于不同的自动化操作，不作为正常退出结果。

在应用退出后，对隔离 synthetic AppData 的 15 个文件逐一记录 SHA-256，同版本 dpkg 重装及卸载后全部文件内容保持。不同版本升级未实测；其他操作系统的安装/升级/卸载仍按实际证据记录。

这些结果是本地候选流程验证，候选 manifest 明确 `commit: uncommitted-p8-local-candidate` 和 `tests_passed: false`，不会通过聚合发布。后续源码可能继续修改；正式安装包必须由最终干净 tag 的四个 native CI job 重新构建、验收和生成真实 commit/hash，绝不通过替换候选 manifest 的 SHA 把本地旧包当成正式资产。
