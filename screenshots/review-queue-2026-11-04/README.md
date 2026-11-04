# 持久人工核对队列浏览器验收

2026-11-04 使用系统 Chromium 运行实际桌面前端，验收脚本通过。参考视口为 1648×928，窄屏为 390×844；全部开启 `prefers-reduced-motion: reduce`。截图直接由浏览器生成，未进行图片编辑。

```sh
MEDIAATLAS_QA_URL=http://127.0.0.1:1432 python scripts/desktop-review-queue-qa.py
```

本次使用明确标注为 `QA 核对媒体` 的合成 IPC 数据。测试账本保存在浏览器 localStorage 中，跨实际页面 reload 还原；它验证前端交互和命令顺序，不代表真实 Rust 数据库持久化、应用进程重启、第三方来源匹配、系统凭据保存或文件写入已经通过。后端 proof、并发与 Root 授权应以本轮 Rust 服务测试为准。

| 场景 | 本次结果与证据 |
| --- | --- |
| 创建核对队列 | 选中 3 项创建 session，携带 root、q、kind、health、sort；[创建后](fixture-created-session-1648.png) |
| 暂缓与重载恢复 | 暂缓第一项后实际 reload，暂缓计数和当前第二项保留，无自动确认或文件执行；[恢复后](fixture-restored-session-1648.png) |
| 配置凭据往返 | 从队列进入设置，手动保存合成凭据，再返回同一 session 和当前媒体；[返回后](fixture-settings-return-1648.png) |
| 失败持久显示 | 合成检索失败写入测试账本；reload 不自动重试或确认；[失败恢复页](fixture-persisted-failure-1648.png) |
| 显式重新核对 | 先点击重新核对，再单独点击检索；检索成功不自动确认；[重新检索后](fixture-explicit-failure-retry-1648.png) |
| 确认顺序 | 断言 `scraper_preview → review_queue_checkpoint → scraper_confirm → review_queue_update(action=confirmed)`，无 `scraper_output_execute`；[显式确认后](fixture-confirmed-proof-order-1648.png) |
| 返回媒体清单 | 原 root、q、kind、health、sort 全部保留 |
| 失效 checkpoint | 显示重新核对提示；不展示确认按钮，不自动重放确认；显式重新核对也不自动检索 |
| Root 禁用或媒体删除 | 隐藏搜索工作区，阻止继续核对；[Root 禁用](fixture-disabled-root-1648.png) |
| 跨页 32 项上限 | 先选 24 项，再整页加 11 项被整体拒绝且仍为 24；逐项补到 32 后创建成功，无静默截断 |
| 窄屏、键盘、减少动态效果 | 390×844 无页面横向溢出；键盘 Enter 暂缓成功，未自动确认；[窄屏全页截图](fixture-keyboard-narrow-390.png) |

[report.json](report.json) 记录执行时 HEAD、工作区差异摘要、队列相关文件和 8 张截图的 SHA-256。执行基线为 `9d6326435190d8c3131d82b36681bdb6d66189d4`，包含本轮尚未提交的队列实现；不能把基线提交本身当成本次实现或最终提交的独立验收记录。设置返回与重载恢复两张截图一致，是同一媒体位置成功恢复后的预期画面。

## 最终实现检查

功能提交 `c81e6ec`。本轮实际执行 `pnpm check`：46 文件 / 397 测试、lint、typecheck、生产 build 全通过；`cargo test --locked`：343 测试通过；Clippy 零警告、Rust 格式与许可证清单检查通过。Rust 测试包含真实持久化、同媒体确认成功证明、目录变更最终事务守卫，以及确认后未记账就崩溃的跨项恢复；并非由浏览器 localStorage fixture 代替。
