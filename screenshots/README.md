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
