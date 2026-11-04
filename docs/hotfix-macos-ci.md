# macOS CI：合并确认与迟到输出预览测试同步

2026-11-04，[main CI 41762423540](https://github.com/susanforme/MediaAtlas/actions/runs/41762423540) 的 [macOS job](https://github.com/susanforme/MediaAtlas/actions/runs/41762423540/job/126352943318) 在前端测试失败：425 项中 424 项通过，`discards a late output preview from a queue item after an external route change` 未找到 `Preview NFO and poster output` 按钮。该 job 的 Rust 步骤未执行；同一提交的 Linux 与 Windows job 成功。日志中的 i18next 赞助提示不是错误。

用例点击确认后直接轮询依赖确认完成才出现的输出按钮，混合了确认提交、React Query 状态通知与输出操作的异步阶段。现在显式控制确认与输出两次 Promise，并等待用户可见的阶段转换：

1. 确认请求开始后，断言仅请求一次、队列导航禁用、输出入口尚未出现且输出 API 未调用。
2. 显式完成确认，等待确认成功反馈，再通过可用的输出按钮请求一次当前媒体的预览。
3. 输出返回前切换到第二项；旧项的预览随后返回，必须仅丢弃一次，不能展示到新项或执行文件写入。

确认点击使用异步 `act`，状态通知仍通过实际可见结果等待。不增加测试超时、不加入固定 sleep 或自动重试，不跳过测试，不放宽产品预览、确认或写入保护；产品代码未改动。

本地针对该文件 41 项通过；使用相同用例体临时参数化执行 20 次，全部通过，临时重复声明已还原。完整 `pnpm check` 通过：50 文件 / 425 测试、lint、typecheck 和生产 build，未跳过测试。远程 macOS 结果须以修复提交的新 main CI 为准，不能将 Linux 本地结果冒称 macOS 已通过。现有 beta.4 标签保持原提交；main 修复不会改变已经触发的 tag 发布源码。
