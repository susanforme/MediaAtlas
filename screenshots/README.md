# 最新：社区流程改进（2026-11-04）

基于 Emby、Jellyfin、MediaElch、tinyMediaManager 和 beets 官方资料，补充快捷问题筛选、详情来源恢复及扫描错误下一步。本轮源码 `3c6ba99`，前端 346 测试和生产检查通过。见 [研究文档](../docs/emby-scraper-workflow.md) 与 [六张本轮截图](community-flow-2026-11-04/README.md)。下列内容保留各阶段历史记录。

---

# 当前：媒体库海报墙与设置标签（2026-11-04）

默认海报墙、一步目录选择、准确区分本地收录与在线资料获取、来源配置继续更新，以及设置五标签已实现。最新源码 `52679c2`，前端 303 项 / Rust 329 项测试及生产构建等检查通过。26 张本轮真实 Chromium 截图和限制见 [本轮图文验收](library-wall-2026-11-04/README.md)。以下保留历史证据，不代表当前页面结构。

---

# 自动媒体库、统一类型与首屏验收（2026-11-03）

本轮后端提交 `553037b`，前端与行为文档提交 `fe44b70`。添加目录后后台扫描、索引并补全可靠资料，本地媒体先显示，无需手动确认入库；成人仅是同一界面的资源类型，设置保留启用开关。后台权限、查询和缓存仍隔离，源媒体保持只读。规则与社区参考见 [自动媒体库](../docs/automatic-library.md)。

重新验证：`pnpm check` 通过 lint、typecheck、**39 文件 / 273 项测试**及生产构建；`cargo test --locked` **328 项测试**通过，Clippy 全目标零警告、Rust 格式检查通过。许可证清单验证 **1001 个依赖 / 1625 条引用**，beta.3 发布 preflight 通过。四组浏览器回归覆盖自动流程、统一类型、人工更正队列与视觉状态。机器可读记录与截图哈希见 [automatic-report.json](automatic-report.json)。

**`v0.1.0-beta.3` 固定在 `67782a2`，仅含 Windows NSIS 提取修复及版本更新，不包含本轮功能。** beta.2 未再次移动；本轮不等待跨平台打包。安装产物状态见 [beta.3 Actions](https://github.com/susanforme/MediaAtlas/actions/runs/41650428810)。

## 生产首屏测量

实际 Chromium 加载 Vite 生产文件，本地 HTTP、4 倍 CPU 降速、每次独立冷上下文，各 5 次。基线为 `375c321`，当前为 `fe44b70`；记录时仅验收脚本与证据尚未提交。

| 指标 | 基线 | 本轮 |
| --- | --- | --- |
| 首屏请求 JavaScript | 584,625 字节 | 487,070 字节（减少 16.7%） |
| 首次内容绘制中位数 | 1016 ms | 144 ms |
| 首页可见中位数 | 886 ms | 492 ms |

本轮主 chunk 487.07 kB / gzip 157.14 kB。9 个延迟路由正常加载，无页面异常；禁用脚本仍能显示静态品牌启动壳。计时受本地负载影响：基线首次内容绘制 664–2428 ms，本轮 108–152 ms；不能将这些浏览器数值写成 Windows 原生启动提升百分比。未引入 SSG；启动壳不含伪造媒体内容。完整采样、资源哈希见 [startup-report.json](startup-report.json)。

## 实际截图

以下均为真实 Chromium 页面截图；桌面 1648×928、窄屏 390×844（部分为完整页面截图），使用明确标记的合成 IPC、媒体与图片 fixture。没有生成、拼接或修补截图。目录选择验证的是原生调用适配契约，截图不证明 OS 文件夹窗口、真实 Provider、keyring 或实际文件修改已通过。

| 场景 | 截图 |
| --- | --- |
| 文件夹选择与自动名称 | [目录](automatic-fixture-directory-1648.png) / [窄屏](automatic-fixture-directory-narrow.png) |
| 添加后可浏览、后台补全 | [媒体库](automatic-fixture-auto-library-1648.png) / [实际任务进度](automatic-fixture-auto-progress-1648.png) |
| 成人仅为资源类型 | [统一浏览](automatic-fixture-adult-type-1648.png) / [窄屏](automatic-fixture-adult-type-narrow.png) |
| 同一详情、工具与目录表单 | [详情](automatic-fixture-adult-detail-1648.png) / [工具](automatic-fixture-adult-tools-1648.png) / [目录](automatic-fixture-adult-roots-1648.png) |
| 首页与减少动态效果 | [首页](automatic-fixture-home-1648.png) / [窄屏](automatic-fixture-home-narrow-reduced-motion.png) |
| 必要状态 | [加载](automatic-fixture-loading-1648.png) / [空](automatic-fixture-empty-1648.png) / [错误](automatic-fixture-error-1648.png) / [缺图](automatic-fixture-missing-1648.png) |
| 键盘与对话框 | [版本选择](automatic-fixture-version-dialog-1648.png) |
| 生产构建与静态启动壳 | [首页](startup-browser-home-1648.png) / [禁用脚本](startup-bootstrap-scripts-disabled-1648.png) |

复现：启动 `pnpm dev:web` 后运行 `scripts/desktop-ui-qa.py`、`scripts/desktop-usability-qa.py`、`scripts/desktop-queue-qa.py`、`scripts/desktop-adult-type-qa.py`；建议设置 `MEDIAATLAS_QA_OUTPUT_DIR` 到独立工作目录，避免覆盖历史截图。生产测量使用 `scripts/desktop-startup-qa.py --baseline <基线生产构建目录> --baseline-ref 375c321 --samples 5 --output <报告路径> --screenshots <截图目录>`。

当前环境缺 GTK/WebKit 开发依赖，未运行原生宿主或完整桌面构建。Rust 服务测试使用临时数据库/合成媒体，浏览器使用测试 IPC；二者不替代端到端原生宿主、实时来源或各平台安装验收。历史记录保留如下，不作为本轮通过证据。

---

# 社区实践优化与二次移动 tag（2026-11-03）

本轮从 `23f61bd` 增量开发，功能源码为 `52f25f2`。重新运行 `pnpm check`：**36 文件 / 250 项测试**、lint、typecheck、前端 build 全部通过；另有 **46 项发布、18 项许可证、7 项 CI 测试**通过，1001 个锁定依赖、1625 条许可引用通过完整验证。发布 preflight 通过。Vite 主 chunk 550.21kB（gzip 177.74kB），构建成功并保留默认体积提示。

依据 [MediaElch 多影片刮削](https://github.com/Komet/MediaElch/blob/master/src/ui/movies/MovieMultiScrapeDialog.cpp)和 [tinyMediaManager 的开源筛选/任务实现](https://gitlab.com/tinyMediaManager/tinyMediaManager/-/blob/devel/src/main/java/org/tinymediamanager/core/movie/tasks/MovieScrapeTask.java)，落实了普通媒体列表的服务器筛选排序、24 项分页、跨页有序多选，以及最多 32 项的逐项核对队列。选择单个详情不丢失批选；筛选/排序变化会清掉失效游标和选择。队列保留配置往返上下文，跳过不会自动确认或写入；字段预设“全选未保护字段 / 全不选 / 仅补空缺”继续保护手改、锁定和已有值。研究来源与取舍见 [工作流文档](../docs/library-workflow.md#2026-11-03-社区实践与逐项核对)。

本轮用户已明确授权移动 `v0.1.0-beta.2` 两次：第一次已从 `a740101` 移至上一轮成果 `23f61bd`；第二次在本次证据提交后移到最终 main，以远端 tag 解引用为准。`v0.1.0-beta.1` 不变。第一次 tag 构建失败暴露了旧的锁依赖许可清单问题，本轮已修复；发布脚本也支持复用已有 Release，并对过期 tag、草稿恢复、混合资产和远端哈希作检查。**移动 tag 不等于安装包已经更新；本次不等待跨平台打包，实际产物状态以 GitHub Actions 为准。**

本轮以下 **7 张图片均为真实 Chromium 截图**，1648×928 与 390×844，Asia/Shanghai。窄屏为完整页面截图，其余为视口截图。测试 IPC、媒体、路径和凭据均明确为合成 fixture，不属于产品库；无真实 Provider、keyring、原生窗口或文件写入验收，没有生成、拼接或修补图片。

| 场景 | 本轮截图 |
| --- | --- |
| 跨页选择后保留顺序 | [批量选择](community-fixture-cross-page-selection-1648.png) |
| 服务端健康筛选、排序与游标复位 | [筛选](community-fixture-server-filters-1648.png) |
| 真实队列位置与手动核对 | [队列](community-fixture-review-queue-1648.png) |
| 凭据设置后回到同一队列项 | [配置返回](community-fixture-queue-settings-return-1648.png) |
| 字段预设（不执行写入） | [字段选择](community-fixture-field-presets-1648.png) |
| 窄屏媒体清单 / 队列 | [窄屏清单](community-fixture-tools-narrow.png) / [窄屏队列](community-fixture-queue-narrow.png) |

复现：启动 `pnpm dev:web`，运行 `python scripts/desktop-queue-qa.py`。前两阶段浏览器回归也已重跑通过，输出使用 `MEDIAATLAS_QA_OUTPUT_DIR` 保存到 work 临时目录，保持历史截图原始来源不被改写。完整结果、来源、哈希与边界见 [community-report.json](community-report.json)。

---

## 前一阶段记录（不代替本轮验证）

# 功能流程优化验收（2026-11-03）

本次从 `main / e928fd3` 继续开发，功能源码为 `7ec38a9`。重新运行 `pnpm check`：lint、typecheck、**32 个测试文件 / 219 项测试**、前端构建通过。原生目录选择的适配器契约测试、Rust 格式检查与 `cargo metadata --locked` 通过。Vite 主 chunk 547.92kB（gzip 176.89kB），构建成功，保留默认体积提示。

本次参考 [MediaElch 目录入门](https://github.com/mediaelch/mediaelch-doc/blob/master/docs/source/quick-start.rst)和[影片刮削流程](https://github.com/mediaelch/mediaelch-doc/blob/master/docs/source/movie/index.rst)：系统选目录、选择本地影片、匹配来源、选择资料并确认保存。只采用交互思路，保留 MediaAtlas 的只读媒体库、成人隔离和显式写入边界。

- 系统目录选择是主入口，自动目录名，手动路径/排除规则折叠。取消、重复点击、晚到响应、手动改名均有行为测试。
- 保存并扫描直接进入对应任务；已保存但扫描失败仅重试扫描，不重复新增目录。唯一根/最近任务可预选，完成后可进入库或媒体工具。
- 详情和媒体工具直接进入当前媒体刮削，预填影片名称、年份和类型；高级标识折叠，来源用友好名称，缺配置时提供实际入口。
- 配置来源/凭据后“返回刮削”保持当前媒体，返回参数仅接受固定入口与合法媒体 ID。凭据仍由用户显式填写保存，启动时不再自动读取环境 Token。
- 元数据、NFO/海报继续分开预览和确认；换目标或取消释放未执行的预览；普通流程防御异常成人响应泄漏。

以下 **8 张图片为本次真实 Chromium 页面截图**（1648×928、390×844，Asia/Shanghai），注入了明确标注的测试 IPC。QA 影片、路径与凭据均为合成测试数据，不进入产品源码或用户库；图片没有生成、拼接或修补。它们验证浏览器流程，**不代表真实原生对话框、真实后端/来源/系统 keyring 或文件写入已经通过**。当前环境缺 GTK/WebKit 开发依赖，未运行原生宿主或打包；既有 tag 未移动。

| 场景 | 本次截图 |
| --- | --- |
| 文件夹选择后的自动名称 / 窄屏 | [目录](usability-fixture-directory-1648.png) / [窄屏目录](usability-fixture-directory-narrow.png) |
| 保存并扫描后的当前任务与候选 | [入库核对](usability-fixture-scan-review-1648.png) |
| 当前影片直接进入刮削 / 窄屏 | [影片上下文](usability-fixture-scrape-context-1648.png) / [窄屏刮削](usability-fixture-scrape-context-narrow.png) |
| 合并预览 | [合并核对](usability-fixture-merge-preview-1648.png) |
| 缺少凭据 / 设置后返回原影片 | [配置入口](usability-fixture-missing-credentials-1648.png) / [返回原影片](usability-fixture-return-from-settings-1648.png) |

复现：启动 `pnpm dev:web` 后运行 `python scripts/desktop-usability-qa.py`（需要 Python Playwright、Pillow 和系统 Chromium）。原视觉回归 `python scripts/desktop-ui-qa.py` 也已重跑通过；本次回归输出通过 `MEDIAATLAS_QA_OUTPUT_DIR=work/regression-captures` 保存到本地临时目录，保留下面上一阶段截图的原始来源记录。精确源码、校验结果、局限与 SHA-256 见 [usability-report.json](usability-report.json)。

---

## 上一阶段视觉重构记录（不作为本次功能验收成绩）

# MediaAtlas 电影式深色重构验收（2026-11-03）

本轮以 `main / 8004e83` 为基线，界面源码为 `2ef08af`。以下 **17 张 PNG 均为本轮 Chromium 实际页面截图**，视口为 1648×928 和 390×844；未使用图像生成、拼接或修补。精确源码 SHA、图片 SHA-256 与验证结果见 [cinematic-ui-report.json](cinematic-ui-report.json)。下方旧版截图及历史成绩不计入本轮验收。

| 本轮场景 | 文件 |
| --- | --- |
| 无桌面宿主（真实浏览器限制） | [desktop-required-1648.png](desktop-required-1648.png) |
| 首页 / 窄屏与减少动态效果 | [fixture-home-1648.png](fixture-home-1648.png) / [fixture-home-narrow-reduced-motion.png](fixture-home-narrow-reduced-motion.png) |
| 媒体库搜索 / 窄屏 | [fixture-library-search-1648.png](fixture-library-search-1648.png) / [fixture-library-narrow.png](fixture-library-narrow.png) |
| 版本选择 / 键盘焦点 | [fixture-version-dialog-1648.png](fixture-version-dialog-1648.png) |
| 加载 / 空 / 错误 / 缺图（含损坏 PNG） | [fixture-loading-1648.png](fixture-loading-1648.png) / [fixture-empty-1648.png](fixture-empty-1648.png) / [fixture-error-1648.png](fixture-error-1648.png) / [fixture-missing-1648.png](fixture-missing-1648.png) |
| 设置 / 窄屏 | [fixture-settings-1648.png](fixture-settings-1648.png) / [fixture-settings-narrow.png](fixture-settings-narrow.png) |
| 媒体工具 / 窄屏 | [fixture-tools-1648.png](fixture-tools-1648.png) / [fixture-tools-narrow.png](fixture-tools-narrow.png) |
| 音乐空状态 | [fixture-music-empty-1648.png](fixture-music-empty-1648.png) |
| 备份维护 / 窄屏 | [fixture-maintenance-1648.png](fixture-maintenance-1648.png) / [fixture-diagnostics-narrow.png](fixture-diagnostics-narrow.png) |

`desktop-required-*` 没有注入 IPC，真实呈现“请在桌面应用中打开”。`fixture-*` 在测试浏览器注入**明确标识的合成 IPC 数据**，海报写有 QA FIXTURE，数据只存在验证脚本，不进入产品源码、不写用户库。它们证明界面渲染、搜索 URL 状态、加载/空/错误/缺图和键盘行为；**不证明真实 Tauri 后端、Provider、系统 App 打开、系统密钥存储或文件写入成功**。Token 验证只使用合成测试值，截图前输入已清空；没有读取或展示环境密钥。

浏览器自动断言通过：所有捕获视口无页面横向溢出；首页四横卡与七海报；无内嵌 video/audio/iframe；成人入口默认隐藏；工具栏搜索导航并复用媒体库唯一搜索状态；多版本选择不自动打开文件，Tab 限制在对话框内，Escape 关闭并恢复触发器焦点；减少动态效果关闭海报位移；Token 仅显式点击保存且立即清空。另有 Vitest 验证成人隔离、锁定后过期响应、只读边界和确认流程。

复现：安装 Python `playwright` / `pillow` 与系统 Chromium，先运行 `pnpm dev:web`，再执行 `python scripts/desktop-ui-qa.py`。可通过 `MEDIAATLAS_QA_URL`、`MEDIAATLAS_CHROMIUM` 指定 URL 和系统浏览器路径。测试浏览器只用于 QA，不是产品依赖；没有捆绑浏览器引擎。

本轮重跑 `pnpm check`：lint、typecheck、29 文件 / 170 测试、前端 build 全部通过。Vite 提示主 chunk 约 535kB（gzip 约 173kB）超过默认 500kB 提示阈值，构建成功。当前环境没有 Rust/Tauri 工具链，本轮未运行原生宿主或桌面打包，既有 tag 未移动。

---

## 以下为历史资料（不计入本轮验收）

# MediaAtlas 0.1.0-beta.1 截图

本轮全部 10 张截图对应应用源码提交 **`d50b4dd03c15e45428c4b455f3e8755d40732a48`**，浏览器 5 张与原生 5 张均已重拍。更新后的界面已移除冗余介绍和页头副标签。截图及报告随后单独提交；生成截图会让 `screenshots/` 变更，不把该时刻整棵工作树称作 clean。

| 页面 | 原生 Linux · 英文 | 浏览器 Mock · 中文 |
| --- | --- | --- |
| 媒体库 | [native-linux-01-library.png](native-linux-01-library.png) | [browser-mock-01-library.png](browser-mock-01-library.png) |
| 媒体工具 | [native-linux-02-tools.png](native-linux-02-tools.png) | [browser-mock-02-tools.png](browser-mock-02-tools.png) |
| 音乐 | [native-linux-03-music.png](native-linux-03-music.png) | [browser-mock-03-music.png](browser-mock-03-music.png) |
| 设置 | [native-linux-04-settings.png](native-linux-04-settings.png) | [browser-mock-04-settings.png](browser-mock-04-settings.png) |
| 备份与维护 | [native-linux-05-maintenance.png](native-linux-05-maintenance.png) | [browser-mock-05-maintenance.png](browser-mock-05-maintenance.png) |

`native-linux-*` 直接捕获 Ubuntu 22.04 容器中真实 Tauri release 应用的 X11 窗口，使用系统 WebKitGTK 4.1、DBus、Xvfb、普通 UID 1000 和独立临时应用数据目录。窗口 1440 × 1080，没有窗口管理器装饰；未禁用 WebKit sandbox 或 TLS 校验，没有 Mock IPC、调试脚本注入或图片修补。合成影片通过此前真实 Root → 扫描 → 预览 → 明确两步确认进入库，本轮复用该数据库及已经验证的备份，仅重拍界面。音乐为空状态，未虚构原生专辑记录。凭据未配置、密码为空，成人入口关闭。完成截图后使用标准 `WM_PROTOCOLS / WM_DELETE_WINDOW` 关窗，应用与 DBus session 退出码为 0。

`browser-mock-*` 是真实 Chromium 页面截图，使用脚本生成的 `Synthetic Demo` 电影、专辑、艺术家、路径和备份响应。它们只证明浏览器渲染，不证明真实后端、OS keyring 或原生安装成功。中文视口 1440 × 1080，长页面采用完整页面截图；不显示真实用户文件或成人图像，没有确认写入、恢复、保存认证或导入调用。

所有图片都已实际查看，内容非空，没有 loading 或错误告警，未使用 ImageGen、拼接或伪造 UI。模式、源码提交、二进制/图片 SHA-256 和验收边界分别见 [native-capture-report.json](native-capture-report.json) 与 [browser-capture-report.json](browser-capture-report.json)。本地原生候选不等同 GitHub Release 可用包，截图不替代 Windows/macOS 实机、媒体播放、实时 Provider 或 keyring 保存验收。

浏览器复现：启动 `pnpm dev:web`（端口 1420），执行 `node screenshots/capture-browser.cjs`。其他环境可设置 `PLAYWRIGHT_MODULE` 和 `CHROMIUM_PATH` 指向已安装的 Playwright/Chromium。脚本记录实际源码提交，并独立检查应用源码输入是否相对该提交发生改变。
