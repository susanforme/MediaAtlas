# Beta 发布矩阵与验证

仅推送 `vX.Y.Z-beta.N` tag 触发 `.github/workflows/release.yml`；main/PR 保持前端与 Rust core 测试 CI。本地 `pnpm build:desktop` 使用 `--no-bundle`。本次发布版本为 `v0.1.0-beta.4`，包含 main 的流程优化、持久核对队列、shadcn 状态展示、双栏实时合并预览、Windows 测试修复与单 EXE 便携 ZIP。版本检查和当前实现验证完成后创建新 tag；不等待原生打包，不以历史 beta 的测试成绩宣称新包通过。既有 beta.1、beta.2、beta.3 保持不动；beta.3 仍指向 `67782a25b6594050dc57014199bc7eaec7f3def3`。

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

每目标均运行 frontend tests、`cargo test --locked --target <native-target>` 和真实 `tauri build --ci --target ... -- --locked`。Linux 安装 WebKitGTK 4.1 等系统开发依赖；实际 Ubuntu 22.04 容器 apt-cache 已确认官方 jammy-updates 提供 libwebkit2gtk-4.1-dev 2.50.4，选择较早发行版以降低 glibc 基线；Windows 使用 MSVC/NSIS，release overlay 显式固定 build.windows.staticVCRuntime=true、bundleVCRuntime=false；实际应用 PE normal/delay imports 必须不依赖 VCRUNTIME/MSVCP 等额外 VC redistributable，便携 ZIP 不以 runner 已装开发工具当运行前提，NSIS 压缩显式采用 BZip2（其完整许可已随 NSIS 材料提供）；这修复完整许可树下 NSIS 3.13 solid zlib 无法被 7-Zip 提取的问题，复现证据见 [Windows hotfix](hotfix-windows-ci.md#nsis-package-extraction-2026-11-03)，macOS 使用原生 Apple 工具链。`tauri build` 负责前端生产编译。未采用插件自动发布，因为需要先验证完整矩阵再集中发布。

上述 label 文档、工作流静态校验以及 Linux 本地 Rust 通过，都不能替代 Windows/macOS 真实编译、启动或安装验收。首次远程 workflow 的 job/产物结果才是各目标构建证据。交互式安装、升级、卸载、用户数据保留、系统 keyring、真实 Provider 访问继续按 P8 平台验收记录；manifest 明确 `installation_tested: false`。

## 产物门槛

`release.py preflight` 严格核对 tag 格式以及根 package.json、desktop package.json、workspace Cargo.toml、Cargo.lock 的所有本项目 crate、Tauri config 的版本一致。未经 P8 统一更新为 beta 版本时，此门槛会失败；不会忽略版本冲突。

`release.py stage` 首先解析原生可执行文件头（PE/ELF/Mach-O），检查真正的 CPU 架构与非空文件；然后逐包验收：

- NSIS：7-Zip 解开安装器，读取内含应用 exe 的 PE 架构与 Windows ProductVersion。NSIS 启动器自身可能为 x86，因此不以安装器启动器架构误判应用目标。
- 便携 ZIP：成员必须恰好为 `MediaAtlas.exe`，不附带目录或零散文件。验证 ZIP CRC、实际 PE 架构、normal/delay CRT imports 和原始 exe SHA-256；然后运行解出的 EXE 导出内嵌许可，逐文件验证完整集合与 SHA-256。NSIS 安装器保持独立资产。
- Debian：`dpkg-deb` 检查包内完整 version/Architecture，再解开检查实际应用 ELF；拒绝额外 ELF、`.so`/DLL/dylib，确保不夹带 GTK/WebKit helpers/shared libraries，依赖系统 runtime。
- AppImage：本次明确 `not_built`。Tauri AppImage 会带入 GTK/WebKit 等原生库；实际归属、逐项版权及同处对应源码分发 gate 尚未完成，不能以 Rust/npm notices 冒称覆盖。preflight/stage 显式禁止重新启用此格式。保留底层提取/架构检查工具供未来开发使用，不代表可分发。
- DMG：`hdiutil` 只读挂载，检查 app 的 Info.plist 版本及实际 Mach-O；无论检查结果如何都卸载。

原生安装包中的 LICENSE-STATUS.md、THIRD_PARTY_NOTICES.md、docs/dependency-licenses.json 与 docs/third-party-licenses/** 全文也需逐文件与源码材料 SHA-256 一致；目录结构保留，避免同名 LICENSE 覆盖。Apple bundle short version 或 Windows product version 在工具链只支持数字版本时允许 SemVer core；完整 beta 版本仍由 tag/config/Cargo/manifest 保持一致。产物以完整版本和 Rust target 重命名，避免不同平台重名。

`stage` 默认只记录 package validation，不宣称自动化测试通过；仅在 workflow 前端与 native Rust core 两步成功后传入 `--tests-passed`。聚合拒绝没有真实测试通过记录的包，本地打包 dryrun 不会自动生成可发布的测试声明。

每目标上传经过检查的包及 manifest，记录 tag、commit、版本、OS、CPU、runner、format、大小、SHA-256、签名状态与实际测试边界。聚合 job 再核对所有必需目标、完整格式集合、路径、重复名、commit/version/target 元数据、实际大小、SHA-256；多余文件、空包、被改动的包或缺少必需格式会阻止 Release。

聚合产物包括所有可用安装包、许可状态/说明/清单、保留全部许可材料目录结构的 license-materials.zip、RELEASE_NOTES.md、release-manifest.json、SHA256SUMS。SHA256SUMS 覆盖除自身以外的每个 Release 资产。GitHub 自动提供的源代码档案链接可由 tag 获取，源码 commit 链接也写入说明。校验和验证字节完整性，不代表签名身份。

## 单 EXE 与许可导出（main，待下次发布）

Windows 便携 ZIP 解压后仅有 `MediaAtlas.exe`，直接运行仍要求系统已有 WebView2。完整许可材料在编译时通过 `include_bytes!` 嵌入 EXE，不依赖开发目录，也不修改已编译的 PE。正常启动不导出任何材料。安装包仍保留原有许可资源；Release 仍单独提供 `license-materials.zip`，无需启动程序也可阅读。

需要本地阅读时，在 PowerShell 显式导出到尚不存在的目录：

```powershell
$p = Start-Process -FilePath .\MediaAtlas.exe -ArgumentList '--export-licenses', 'licenses' -Wait -PassThru
$p.ExitCode
```

父目录须已存在；目标目录或文件已存在时拒绝覆盖。成功返回 0，参数或写入错误返回 2，导出路径不启动 GUI、数据库或网络；写入中断可能留下部分新目录，应检查后换一个新目录重试。使用 `Start-Process -Wait -PassThru` 可可靠等待 Windows GUI 子系统 EXE 并读取退出码。此 flag 同样支持 macOS/Linux 原生可执行文件。

`test_portable_notices.py` 在三平台 main CI 编译实际 std-only 嵌入生成器和导出模块，删除所有 fixture 许可源文件后执行导出，确认字节与文件集合完全一致。它也检查非法参数、已有目录不覆盖及 ZIP 拒绝路径。此检查不等同完整 Tauri 宿主构建；正式 Windows `stage` 仍须运行真实打包 EXE 的导出并检查真实 PE/CRT/哈希。

本轮本地验证：52 项发布工具测试全部通过（包含真实 NSIS 提取回归和新导出测试）；导出模块内部 3 项 Rust 测试执行通过。另有 7 项 CI 工具测试、Rust 格式检查、前端 lint、1001 依赖 / 1625 引用的许可校验与 beta.3 preflight 通过。未构建新的完整 Windows Tauri EXE，正式包验证由下一次发布运行完成。

## 发布权限与恢复

默认 `contents: read`，仅聚合发布 job 获得 `contents: write`。checkout 禁止保留 git credentials。只有全部必需构建成功且聚合校验通过，才先创建 draft prerelease，上传完整资产，最后公开；`--latest=false` 不替换正式版 latest。上传中断会留下未公开 draft，而非公开缺少资产的 Release。现有同 tag Release 不会被脚本静默覆盖；修复失败构建后可重跑 workflow，若已留下 draft，维护者应先核对 draft 并删除失败 draft 后重跑发布 job。不要覆盖已发布 tag；修复已公开 beta 时递增 beta.N。

不启用自动更新、updater artifacts、证书签名或 Apple notarization。未配置凭据时不会伪造签名；macOS 可能含工具链用于本机运行的 ad-hoc 签名，但不等同可信分发签名。Windows SmartScreen、macOS Gatekeeper 可能提示，发布说明明确这一状态。Windows NSIS 显式使用官方 downloadBootstrapper：系统缺少 WebView2 runtime 时在线安装，不捆绑 fixed/offline Chromium/WebView2 engine；ZIP 要求已有 WebView2 runtime。macOS 实际 app 用 otool 验证链接系统 WebKit/WKWebView，禁止内嵌 Chromium/WebKit framework；Debian 包显式依赖系统 WebKitGTK 4.1、GTK3、libdbus-1 与 xdg-utils，实际 ELF 的 DT_NEEDED 必须有 WebKitGTK/GTK/libdbus，readelf symbol versions 的最高 GLIBC 不得超过 2.35；AppImage 本次不分发，缺少其实际原生库归属和对应源码分发 gate；Ubuntu 22.04 构建的 glibc 基线为 2.35，不承诺旧发行版兼容。ffmpeg/ffprobe 仅为开发 fixture 工具，应用不调用或捆绑它们；拆包校验显式拒绝夹带这两个可执行文件。AppImage 候选 overlay 显式关闭 bundleMediaFramework，但此设置不能替代其未完成的原生库分发审查，也不使该格式进入本次矩阵。

项目自身尚未指定开源许可，不因公开源码或 beta 二进制而新增权利授予；保持 LICENSE-STATUS.md 的准确说明。依赖保留其自身许可，P8 维护真实的 THIRD_PARTY_NOTICES.md、docs/dependency-licenses.json 及 docs/third-party-licenses/** 内所需完整许可材料。

## 本地验证与外部前提

```sh
python3 -m unittest discover -s scripts/release -p 'test_*.py' -v
actionlint .github/workflows/release.yml
python3 scripts/release/release.py preflight --tag v0.1.0-beta.4
```

首次准备时 24 个 verifier 测试与 actionlint 1.7.12 已在本地通过，合并 Tauri release 配置通过官方 JSON schema 验证；preflight 需 P8 的版本统一及真实许可文件齐全。模拟 fixture 测试验证发布校验器的拒绝路径和完整汇总，不把 fixture 当作 MediaAtlas 安装包通过证据。

当前 managed 环境的 enforced HTTP allowlist 缺少 `api.github.com`，代理曾在 CONNECT 阶段返回 403，因此本地 GitHub Actions/Release API 监控受网络策略阻挡；官方 github.com 的公开 Actions/run/job 网页仍可用于只读状态监控，未登录的详细 step 日志端点会返回 404，不能把它解释为没有失败；这不能推断为缺少 GitHub token 或仓库权限。git push 已可用。待用户发布包含 api.github.com 的环境配置且状态实际 enforced 后，再以正常 `gh` 只读命令确认运行结果；不打印凭据、不绕过代理、不反复请求同一个已被策略禁止的地址。GitHub-hosted runners 使用其自己的网络，当前本地 API 监控阻塞并不直接证明远端 workflow 不能运行。

2026-11-03 原 main `8d55797` 的 Windows 失败及后续 macOS fixture 失败已通过独立 hotfix 修复；最终 `1e80669` 的 [真实三平台 CI run 41612577348](https://github.com/susanforme/MediaAtlas/actions/runs/41612577348) 已整体 COMPLETED/SUCCESS（8m36s）。Windows job `125837899288`、Linux job `125837898742`、macOS job `125837899119` 的前端与 Rust core 全部成功；macOS Rust 于 09:45:47 UTC 完成。此证据针对该独立 hotfix，P8 新源码仍需其最终 main CI，再允许创建 beta tag。不得把较早 Linux 本地绿灯当作 Windows/macOS 证据。


## Linux 本地候选联调证据

标准 Ubuntu 22.04 容器以非 root UID 1000 构建过未提交的 P8 本地候选；没有 source 宿主 activate.sh，PKG_CONFIG_SYSROOT_DIR、PKG_CONFIG_PATH、LD_LIBRARY_PATH、LIBRARY_PATH、RUSTFLAGS、CARGO_ENCODED_RUSTFLAGS 均 unset，三个 pkg-config libdir 均来自 `/usr/lib/x86_64-linux-gnu`。managed proxy/只读 CA 保持有效，未关闭 TLS 或 WebKit sandbox。

实际候选 `.deb` 为 16,095,924 字节、amd64、0.1.0-beta.1；真实 ELF DT_NEEDED 包含系统 WebKitGTK 4.1、GTK3、libdbus-1.so.3，最高 GLIBC symbol version 为 2.34，不超过声明的 2.35；拆包架构/非空/版本/许可字节以及没有多余 engine/native ELF 全部通过。实际 dpkg 首次安装成功；真实后端/隔离 AppData 的原生 UI 启动、备份和五页截图由 screenshots 记录。标准 WM_DELETE_WINDOW 正常关闭返回 0；早期 XDestroyWindow 的 133 属于不同的自动化操作，不作为正常退出结果。

在应用退出后，对隔离 synthetic AppData 的 15 个文件逐一记录 SHA-256，同版本 dpkg 重装及卸载后全部文件内容保持。不同版本升级未实测；其他操作系统的安装/升级/卸载仍按实际证据记录。

这些结果是本地候选流程验证，候选 manifest 明确 `commit: uncommitted-p8-local-candidate` 和 `tests_passed: false`，不会通过聚合发布。后续源码可能继续修改；正式安装包必须由最终干净 tag 的四个 native CI job 重新构建、验收和生成真实 commit/hash，绝不通过替换候选 manifest 的 SHA 把本地旧包当成正式资产。
