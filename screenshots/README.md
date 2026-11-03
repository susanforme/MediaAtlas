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
