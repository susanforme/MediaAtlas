# MediaAtlas 0.1.0-beta.1 截图

本目录的 `browser-mock-*` 图片是 **真实 Chromium 页面截图 + 明确合成的 Mock IPC 演示数据**，不是原生桌面、实际媒体扫描、安装、OS keyring 或 Provider 联网验收证据。截图由当前应用实际 React 页面渲染，未使用 ImageGen、图像修补、拼接或伪造 UI。版本 `0.1.0-beta.1`，语言 `zh-CN`，视口 1440 × 1080，长页面采用完整页面截图。

所有 `Synthetic Demo` 电影、专辑、艺术家、路径和备份记录均为脚本生成，不对应用户文件；海报缺失时保留应用真实占位样式。成人入口关闭，认证状态未配置，密码输入为空。维护截图只执行合成备份预览，没有确认写入或恢复。

| 页面 | 实际截图 |
| --- | --- |
| 只读媒体库与筛选 | [browser-mock-01-library.png](browser-mock-01-library.png) |
| 独立媒体工具与整理配置 | [browser-mock-02-tools.png](browser-mock-02-tools.png) |
| 独立音乐媒体库 | [browser-mock-03-music.png](browser-mock-03-music.png) |
| 来源认证、语言与隐私设置 | [browser-mock-04-settings.png](browser-mock-04-settings.png) |
| 数据库备份预览、安全诊断与缓存维护 | [browser-mock-05-maintenance.png](browser-mock-05-maintenance.png) |

每张图片都已实际查看，确认内容非空、没有 loading/error 提示；自动脚本同时断言页面无 JavaScript 错误、无错误告警、密码为空、没有确认/执行/保存/导入调用。机器可读记录见 [browser-capture-report.json](browser-capture-report.json)。

复现：启动当前仓库 `pnpm dev:web`（端口 1420），安装 Playwright/Chromium 后执行 `node screenshots/capture-browser.cjs`。脚本默认使用本次执行环境的 `/opt/codex/cua_node/lib/node_modules/playwright` 和 `/usr/bin/chromium`；其他环境可设置 `PLAYWRIGHT_MODULE` 和 `CHROMIUM_PATH` 指向现有安装。Mock 仅注入截图用浏览器进程，不改产品代码或真实桌面凭据。

## 真实原生 Linux 应用

新增 `native-linux-*` 为 **真实 Tauri release 应用 + 真实 Rust/SQLite 后端** 截图。使用标准 Ubuntu 22.04 容器中的系统 WebKitGTK 4.1、DBus 和 Xvfb `:99`，以 UID 1000 运行，窗口 1440 × 1080、`en-US`。未禁用 WebKit sandbox 或 TLS 校验，没有 Mock IPC、调试脚本注入或图像修改。ImageMagick 直接捕获实际应用 X11 窗口；未运行窗口管理器，图片不含 OS 标题栏。

这次使用临时独立 `XDG_DATA_HOME`。通过真实 UI 添加 `Synthetic Demo Library` → 扫描临时生成的 dummy 视频及 NFO → 查看候选 → 明确两步确认入库，得到一条 `Synthetic Demo Mountain Journey`；源字节保持原样。dummy 文件仅用于索引演示，不是可播放影片，未尝试播放。音乐页展示该真实临时库的空状态，没有植入虚构的原生专辑数据。设置页凭据未配置、输入为空，成人入口关闭。

| 原生页面 | 实际截图 |
| --- | --- |
| 已确认的合成 NFO 资源 | [native-linux-01-library.png](native-linux-01-library.png) |
| 真实本地文件详情与整理配置 | [native-linux-02-tools.png](native-linux-02-tools.png) |
| 独立音乐库真实空状态 | [native-linux-03-music.png](native-linux-03-music.png) |
| 实际来源认证设置（无凭据） | [native-linux-04-settings.png](native-linux-04-settings.png) |
| 真实备份成功状态 | [native-linux-05-maintenance.png](native-linux-05-maintenance.png) |

维护页实际执行 Preview → Confirm 生成 602,112 字节 SQLite 一致备份，界面显示 `Backup saved.`；额外核验 manifest SHA-256、`PRAGMA integrity_check=ok`、一条合成媒体记录及源文件字节保全。五张原生图片均实际查看，内容非空，没有 loading 或错误告警。详情、图片哈希与二进制 SHA-256 见 [native-capture-report.json](native-capture-report.json)。

这些原生图来自当前本地尚未提交的 `0.1.0-beta.1` 候选，不能替代 GitHub Release 产物、Windows/macOS 实机、媒体播放、实时 Provider 或 keyring 认证验收。初始容器缺少 UID 1000 的 passwd 条目，DBus 曾发出认证警告；随后已建立标准普通账户，但本次未测试保存 keyring。截图完成后首次使用 `xdotool windowclose`（实际为 `XDestroyWindow`）直接毁窗，触发 GDK `BadWindow`、退出码 133，属于关闭自动化方法错误。随后使用同一临时数据目录重启真实应用，发送标准 `WM_PROTOCOLS / WM_DELETE_WINDOW` X11 ClientMessage 模拟正常关窗，应用及 DBus session **退出码 0**，进程已停止且合成数据/备份保留。这个复核不需要窗口管理器，也未修改产品或禁用 sandbox；报告保留了两次测试的准确边界。
