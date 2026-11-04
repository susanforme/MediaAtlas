# 窗口操作区、媒体库分类与 Sonner

2026-11-04 在真实 Chromium 中运行桌面前端，以明确标记的测试 IPC 和窗口 API 替身验收。所有海报均为 `QA FIXTURE / TEST ARTWORK`，不是用户媒体、真实影片或在线刮削结果。

执行：先运行 `pnpm --filter @mediaatlas/desktop dev --port 1432`，再从仓库根运行 `python scripts/desktop-chrome-notification-qa.py`。本轮结果：通过；源码及截图 SHA-256、SDK 命令记录见 [report.json](report.json)。

| 场景 | 真实运行截图 |
| --- | --- |
| Windows 样式的右上窗口操作区、可点击媒体库与二级分类 | [1648×928](fixture-windows-library-1648.png) |
| 分类加搜索、排序、缺海报筛选后，父子导航保持选中 | [1648×928](fixture-library-filter-selection-1648.png) |
| 仅右侧内容滚动，侧栏位置与底部设置固定 | [1648×928](fixture-fixed-sidebar-scroll-1648.png) |
| 窗口操作失败显示安全文案，支持重试 | [1648×928](fixture-window-error-recovery-1648.png) |
| 多版本打开请求成功关闭选择框，Sonner 提示，无重复行内成功文字 | [1648×928](fixture-sonner-native-open-1648.png) |
| 窄屏窗口控件独立顶行、导航键盘焦点 | [390×844](fixture-windows-narrow-keyboard-390.png) |
| macOS Overlay 预留系统控制点位置 | [1648×928](fixture-macos-overlay-spacing-1648.png) · [390×844](fixture-macos-narrow-390.png) |

同时验证电影、电视剧、动画分类切换，音乐与详情页的媒体库父级选中，点击媒体库清除分类，停用成人类型时不显示该分类；窗口最小化防重复、最大化/还原状态、焦点变化、关闭命令；交互区域不触发拖动，工具栏与窗口控件不重叠；Sonner 单实例、中文通知区与关闭名称、键盘关闭、减少动态效果；无页面脚本错误或横向溢出。

这是浏览器前端与 SDK 调用验证。窗口 API 被测试替身接收，最小化和关闭没有改变实际系统窗口，也没有调用系统关联应用。macOS 截图只展示预留位置，不绘制伪造的三色按钮；原生交通灯、拖动/双击行为以及真实 Windows/macOS 窗口仍需原生宿主验收。本组不验证真实 Provider、凭据、NFO 输出或文件写入。
