# MediaAtlas 当前界面验证

这里只保留仍能说明当前流程的两组真实浏览器截图；过时的概览/旧界面、重复 fixture/native 截图、旧用户走查和重复报告已移除。截图以各组记录的源码提交为边界，不能把旧成绩当成本轮通过。

| 范围 | 记录 |
| --- | --- |
| 处理快捷筛选、详情恢复、后台处理中及扫描失败/取消 | [社区流程](community-flow-2026-11-04/README.md) · [来源与哈希](community-flow-2026-11-04/report.json) |
| 默认媒体墙、目录选择、来源配置、设置标签与必要状态 | [媒体墙与设置](library-wall-2026-11-04/README.md) · [来源与哈希](library-wall-2026-11-04/report.json) |

两组为实际 Chromium 页面捕获，包含 1648×928、390×844 和明确标注的合成媒体/图片/IPC。没有生成、拼接或修补截图；不代表真实 Tauri 宿主、系统选择器、keyring、在线 Provider、安装或文件修改已经通过。

## 当前 P0

新的逐项核对队列 P0 正在实施，完成后会在 `review-queue-2026-11-04/` 刷新实际截图及验收；这里尚未将该轮记为通过。当前采用的流程与后续工作见 [Emby 流程基准](../docs/emby-scraper-workflow.md)，平台状态见 [发布矩阵](../docs/release-matrix.md)。

## 复现

启动 `pnpm dev:web`，使用 Python Playwright、Pillow 和系统 Chromium：

```bash
MEDIAATLAS_QA_OUTPUT_DIR=work/qa/library-wall python scripts/desktop-library-wall-qa.py
MEDIAATLAS_QA_OUTPUT_DIR=work/qa/community-flow python scripts/desktop-community-flow-qa.py
```

`MEDIAATLAS_QA_URL` 可指定运行地址，`MEDIAATLAS_CHROMIUM` 可指定浏览器。临时复核输出到 `work/`，只有与当前源码匹配、已检查的证据才保存在本目录。QA 浏览器不是产品运行时依赖，应用不捆绑浏览器引擎。
