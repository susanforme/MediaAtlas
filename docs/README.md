# MediaAtlas 文档索引

当前产品流程以 [Emby 与成熟刮削工具流程改进](emby-scraper-workflow.md)和[自动媒体库](automatic-library.md)为准；视觉及前端约束见根 [design.md](../design.md)。历史实施计划、阶段验收成绩和旧 UI 操作指南已移除。按阶段命名的文档仍保留有效数据结构、协议和安全边界，不表示产品停留在该阶段；后续新增能力以实际代码和追加迁移为准。

| 范围 | 当前依据 |
| --- | --- |
| 流程与异常处理 | [Emby 流程基准](emby-scraper-workflow.md)、[自动媒体库](automatic-library.md) |
| 分层、IPC 与隐私 | [架构](architecture.md)、[架构决策](adr/) |
| 模型与存储 | [领域模型](domain-model.md)、[数据库](database-schema.md)、[存储接口](phase-1-contract.md) |
| 浏览、扫描与任务 | [媒体库 IPC](phase-2-contract.md)、[持久化核对与实时合并](review-queue.md) |
| 来源与访问 | [Provider 契约](provider-contract.md)、[访问与凭据](provider-access.md) |
| 影视与文件输出 | [影视刮削](phase-4-contract.md)、[安全旁车输出](media-output.md) |
| 成人与音乐边界 | [成人契约](phase-5-contract.md)、[音乐契约](phase-6-contract.md) |
| 文件组织、监听与系统打开 | [组织与消费契约](phase-7-contract.md) |
| 备份、恢复与发布 | [维护说明](maintenance.md)、[维护/分发契约](phase-8-contract.md)、[发布矩阵](release-matrix.md)、[Windows 修复证据](hotfix-windows-ci.md)、[macOS CI 测试同步修复](hotfix-macos-ci.md)、[macOS 安装包资源签名](hotfix-macos-package.md) |
| 实际界面验证 | [当前截图记录](../screenshots/README.md) |

## 许可材料

[dependency-licenses.json](dependency-licenses.json)与整个 `third-party-licenses/` 是构建和发布的必要输入，保留原始许可、NOTICE、版权与必要源包，不能因旧日期删除或改写。分发要求见 [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)和 [LICENSE.md](../LICENSE.md)。
