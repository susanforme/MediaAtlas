# NFO 只读预览

安装包将 MediaAtlas 注册为 `.nfo` 的可选查看器（`Viewer` / `Alternate`），不自动抢占系统默认应用。可通过系统“打开方式”关联，也可从应用的 NFO 入口选择单个文件。

关联启动优先判断 NFO 模式，跳过媒体库数据库、任务恢复、监控和扫描初始化；普通启动才执行原媒体库初始化。Windows / Linux 的关联参数与 macOS 的文件打开事件使用同一只读解析入口。macOS 若文件事件晚于普通启动模式判定，会在已经启动的应用中打开预览；不会因该文件执行导入、刮削或旁车写入。

预览只读取系统关联或原生文件选择器明确给出的单个文件，不提供接受任意路径的读取 IPC，不遍历同目录，不使用媒体库权限、不读取关联图片或远程 URL。最大 2 MiB，要求 UTF-8；XML 复用本地 NFO 解析器，拒绝 DTD / 外部实体，纯文本 NFO 按文本显示。所有字符串以 React 文本输出，不执行 NFO 中的 HTML。终端符号链接不跟随，读取和大小检查基于同一个文件句柄。原文件不被改写，预览不入库，也不持久化文件内容。

macOS 包声明支持 `zh-Hans` 和 `en`，原生选择器标题使用当前应用语言。[Apple 的 bundle 本地化声明](https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundlelocalizations)允许系统选择中文资源；原生固定按钮和侧栏仍由 macOS 为该应用选择的语言控制。Tauri 当前对话框 API 没有设置文件选择器确认按钮文字或运行时系统语言的接口。若应用内切换与系统面板语言不同，可在 macOS 的“语言与地区 → 应用程序”中选择 MediaAtlas 的语言；这里不修改系统全局语言或用户偏好。[Tauri 对话框源码](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/dialog/src/commands.rs)

本轮遵守用户约束，仅静态修改；安全与界面测试已编写但未执行，未运行构建、安装包或实际系统文件关联验收。
