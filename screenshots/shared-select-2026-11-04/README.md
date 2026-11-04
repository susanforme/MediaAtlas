# 共享 Select 验收

2026-11-04，实际 Chromium 运行截图，1648×928 / 390×844，使用明确标记的测试 IPC 与合成媒体；不是用户媒体库或原生 macOS/Windows 截图。

- [媒体库](fixture-shared-select-library-1648.png)
- [键盘焦点](fixture-shared-select-focus-1648.png)
- [禁用与无效样式](fixture-shared-select-states-1648.png)：临时 DOM 状态，用于验证共享控件样式。
- [空库](fixture-shared-select-empty-1648.png)
- [窄屏](fixture-shared-select-narrow-390.png)

运行：`MEDIAATLAS_QA_URL=http://127.0.0.1:1432 python scripts/desktop-select-qa.py`。验证 WebKit/标准 appearance 重置、单箭头、字号、键盘选值更新实际 URL、焦点、禁用、无效状态、语言、减少动态效果和无横向溢出。源码及图片 SHA-256 见 [report.json](report.json)。

保留原生 select 的受控值和 change 语义；展开菜单仍由平台管理。当前验证没有替代 macOS WKWebView/Windows WebView2 实机验收。
