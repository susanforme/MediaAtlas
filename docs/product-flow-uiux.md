# MediaAtlas 产品流程与 UI/UX

更新：2026-11-05 · 代码基线：`b99533c`

本文按当前路由、页面和后台工作流整理；视觉参数引用仓库 `design.md`，不代表已经完成本轮视觉验收。

MediaAtlas 围绕本地媒体文件提供浏览、资料补全、核对和输出。日常路径是：**添加目录 → 后台收录 → 浏览海报墙 → 查看详情 → 通过系统应用打开**。缺少资料不阻止浏览，歧义和失败进入可处理状态。

## 1. 信息架构

```mermaid
flowchart LR
    Start[启动 MediaAtlas] --> Library[媒体库：默认海报墙]
    Library --> Types[电影 / 剧集 / 动画]
    Library --> Music[音乐]
    Library --> Adult[成人资源类型：设置启用后可见]
    Types --> Detail[媒体详情]
    Adult --> AdultDetail[隔离的媒体详情]
    Music --> Album[专辑 / 艺术家详情]

    Nav[侧栏工作入口] --> Roots[媒体目录]
    Nav --> Tools[媒体工具]
    Nav --> Tasks[任务中心]
    Roots --> Scanner[扫描详情]
    Tools --> Scanner
    Tools --> Scraper[刮削与合并工作台]
    Tools --> Organize[文件组织 / 健康检查 / 监听管理]

    Bottom[侧栏底部] --> NFO[NFO 只读预览]
    Bottom --> Settings[设置]
    Settings --> General[通用：主题 / 语言 / 媒体类型]
    Settings --> Auth[来源认证]
    Settings --> Providers[数据来源]
    Settings --> Network[网络检查器]
    Settings --> Diagnostics[连接诊断：备份 / 恢复 / 缓存 / 诊断导出]
```

| 入口 | 当前界面与行为 |
| --- | --- |
| `/` → `/library` | 默认媒体库，无概览页；海报墙与列表可切换 |
| `/media/$mediaId` | 媒体详情；成人资源通过类型上下文进入隔离查询 |
| `/music` | 音乐库；进一步进入专辑、艺术家和音乐工具 |
| `/roots` | 统一目录管理与添加表单 |
| `/tools` | 本地媒体选择、刮削、健康检查；选择媒体后显示组织等操作 |
| `/scanner`、`/scraper` | 从目录、工具、详情和处理入口进入的工作页 |
| `/tasks` | 扫描与来源任务日志、状态、取消及恢复入口 |
| `/nfo` | 选择并只读预览 NFO；也支持系统文件关联启动 |
| `/settings` | 通用、来源认证、数据来源、网络检查器、连接诊断五个标签页 |

音乐目前属于媒体库侧栏分组。旧 `/adult` 等入口重定向到统一媒体库、工具和详情，不再形成独立成人中心；来源、网络和诊断没有旧式独立业务路由。

## 2. 添加目录与自动更新

```mermaid
flowchart TD
    Add[添加媒体目录] --> Picker[直接打开系统文件夹选择器]
    Picker --> Choice{是否选择目录？}
    Choice -->|取消| Unchanged[不保存，不改变已有输入]
    Choice -->|选择| Form[确认名称与内容类型]
    Form --> Config[按需调整来源顺序、排除规则和 NFO 图片模式]
    Config --> Save[添加到媒体库]
    Save --> Saved{目录是否保存成功？}
    Saved -->|否| SaveError[显示错误，保留可修正表单]
    SaveError --> Form
    Saved -->|是，且目录启用| Background[启动后台更新，进入该目录媒体列表]
    Background --> Index[扫描文件并建立索引]
    Index --> Visible[实际本地媒体先可浏览]
    Index --> Local{是否有有效本地 NFO？}
    Local -->|有| Read[读取本地资料，保留已有 NFO]
    Local -->|无| Source{来源是否可用？}
    Source -->|未启用或缺凭据| Setup[显示资料未获取，提供设置入口]
    Source -->|可用| Match{按目录来源顺序匹配}
    Match -->|可靠匹配| Merge[自动补全可靠资料]
    Match -->|歧义或未匹配| Review[待核对，保留本地浏览]
    Match -->|请求失败| Failed[显示失败与重试入口]
    Merge --> Missing[生成缺失 NFO，保护手改与锁定字段]
    Missing --> Mode{目录图片模式}
    Mode -->|默认 HTTPS| Remote[在 NFO 中保存 HTTPS 图片引用]
    Mode -->|已明确保存离线图片| Offline[输出配套图片，NFO 使用相对路径]
    Read --> Result[分别汇报本地收录与资料获取结果]
    Remote --> Result
    Offline --> Result
    Setup --> Result
    Review --> Result
    Failed --> Result
    Background -->|保存成功但更新启动失败| Retry[仅重试更新，避免重复新增目录]
    Retry --> Background
```

- 手填路径和排除规则在高级配置中；选目录自动填名称，用户改名后保留其输入。
- 目录可以独立选择多个来源并排序；空来源列表只使用本地资料。目录设置不自动启用全局来源，也不保存凭据。
- 停用目录可保存配置，不立即启动后台更新。
- 有效本地 NFO 优先，不要求特定来源；已有 NFO 默认保留，不因后台更新再次自动刮削。
- 普通视频目录默认监听文件变化，尊重用户关闭监控的选择。已识别媒体删除 NFO 后，使用库内可靠资料按目录模式补回；待核对项和可选联网修图不阻断后续事件或补回。
- 离线图片授权只适用于明确保存该模式的目录和新生成的缺失 NFO；不为已有 NFO 补写图片，也不授权音频标签、移动或覆盖。

## 3. 浏览、详情与系统打开

```mermaid
flowchart LR
    Wall[海报墙] --> Filter[搜索 / 类型 / 排序 / 扩展筛选]
    Filter --> Results[服务端查询与分页]
    Results --> Card[整张海报进入详情]
    Results --> List[列表视图：标题打开与详情入口]
    Card --> Detail[详情：简介 / 人员 / 来源 / 文件 / 状态]
    List --> Detail
    Detail --> Open[通过系统应用打开]
    List --> Open
    Open --> Count{可打开的文件或版本}
    Count -->|一个| Request[提交系统打开请求]
    Count -->|多个| Pick[版本或文件选择框]
    Pick --> Request
    Request --> Outcome{系统请求结果}
    Outcome -->|成功| Done[关闭选择框，反馈请求已提交]
    Outcome -->|失败| Again[显示错误，保留选择并允许重试]
    Again --> Pick
    Detail --> Recovery[补资料 / 核对匹配 / 处理 NFO 状态]
    Recovery --> Work[进入带当前媒体目标的工作台或恢复流程]
```

浏览页默认保持紧凑：搜索进入顶部工具栏，类型、排序和视图切换位于控制行；目录、健康状态、类型标签、人员、制作方和年份等扩展筛选按需展开。筛选、排序、分页和视图由 URL 查询状态承载。

电影与剧集使用专门的详情构图；剧集呈现真实季、集和文件信息。其他类型沿用通用详情。评分、年份、简介、人员和图片来自实际资料，缺失时省略或显示明确缺失状态；不存在观看进度或“继续观看”数据。

系统打开的成功反馈只代表操作系统接收请求，不代表已经开始播放。

## 4. 人工刮削、合并与导出

```mermaid
flowchart TD
    Entry[媒体详情 / 工具选择 / 待核对任务] --> Target[绑定真实本地媒体，预填标题、年份与类型]
    Target --> Ready{所选来源是否可用？}
    Ready -->|否| Settings[进入来源配置，保留媒体与队列上下文]
    Settings --> Explicit[手动保存凭据，显式启用来源]
    Explicit --> Resume[返回工作台或继续目录更新]
    Resume --> Ready
    Ready -->|是| Search[检索来源候选]
    Search --> Select[用户选择候选]
    Select --> Workspace[双栏合并工作区]
    Workspace --> Fields[选择字段 / 仅补空缺 / 查看前后差异]
    Fields --> Preview[即时前端投影 + 自动后端校验预览]
    Preview --> Valid{预览有效且有真实变更？}
    Valid -->|否| Block[禁止确认，修正选择或重试]
    Block --> Fields
    Valid -->|是| Confirm[明确确认元数据合并]
    Confirm --> Database[更新库内资料，自动策略可生成缺失 NFO]
    Database --> Export[按需预览 NFO 与图片输出]
    Export --> Compare[检查目标路径、保护状态与新旧内容]
    Compare --> Write[明确确认写入]
    Write --> Finish[反馈结果，刷新媒体与任务状态]
    Compare -->|取消| Discard[释放输出预览，不执行写入]
```

| 工作区 | 用户看到和控制的内容 |
| --- | --- |
| 左栏 | 本地基础资料、当前补充来源、字段选择、仅补空缺 |
| 右栏 | 合并后的媒体展示、前后对比、逐字段明细 |
| 确认区 | 真实变更汇总；等待校验、失败、零变更或预览过期时不可确认 |
| 文件输出区 | 目的路径、受保护文件、新旧内容、执行与取消 |

- 确认资料合并与确认文件输出是两个操作。更新已有同名 NFO 需要用户主动再次刮削并确认导出。
- 字段锁定、手改保护和仅补空缺在最终写入时继续生效。
- 切换媒体或候选、取消与离开使未确认预览失效，晚到响应不能恢复旧选择。
- 普通视频可批选最多 32 项进入逐项核对；上一项、下一项、跳过不自动确认，不默认选首候选。另有持久化核对会话保存处理状态。
- 确认或写入期间禁止切换条目；来源设置往返保留当前媒体和队列。

## 5. 工具、音乐与 NFO 支线

```mermaid
flowchart LR
    Tools[媒体工具] --> Local[筛选和选择真实本地媒体]
    Local --> Organize[选择目标目录、命名模板和策略]
    Organize --> Plan[预览路径、文件数、空间和冲突]
    Plan --> Execute[明确确认组织操作]
    Execute --> History[查看历史与日志]
    History --> Restore[恢复预览 → 明确确认恢复]
    Tools --> Health[检查健康问题 → 进入对应媒体处理]
    Tools --> Watch[查看与配置目录监听]

    Music[音乐库] --> Album[专辑详情 / 艺术家 / 曲目与来源]
    Music --> MusicTools[音乐工具]
    MusicTools --> Edition[搜索并选择发行版本 → 查看内容 → 导入资料]
    MusicTools --> Link[关联本地音频文件]
    Link --> Tags[选文件与标签字段 → 预览 → 确认写入]
    Tags --> TagHistory[写入历史 / 恢复预览]

    NFO[NFO 入口或系统文件关联] --> Read[只读解析与内容预览]
    Read --> Safe[不加载引用的任意图片、HTML 或外部实体]
```

日常浏览和自动更新保持源媒体只读。文件组织提供保留、移动、复制和硬链接策略，是独立的显式操作，必须先预览再确认；音频标签写入同样有独立流程，不由目录自动任务授权。

NFO 文件关联启动使用独立只读模式，不初始化媒体库、扫描或写入服务。音乐的发行版本资料导入与本地文件关联分开；导入资料不等于已拥有或关联本地音频。

## 6. 状态反馈与恢复路径

| 当前情况 | 界面反馈 | 下一步 |
| --- | --- | --- |
| 未添加目录 | 空媒体库 | 添加目录，直达系统选择器 |
| 有目录但暂无文件 | 与“无搜索结果”区分的空状态 | 查看更新状态或管理目录 |
| 正在扫描、索引或获取资料 | 后台活动与任务阶段、真实计数 | 继续浏览，按需查看任务 |
| 已收录但来源未配置 | 明确“资料未获取” | 配置凭据、显式启用并继续更新 |
| 有歧义或未匹配 | 待核对状态与处理入口 | 人工检索、选择候选、核对合并 |
| 来源请求失败 | 安全错误信息与可用恢复动作 | 重试、改检索条件或配置来源 |
| NFO 缺失或输出失败 | 独立 NFO 状态 | 管理目录、重试更新或按流程导出 |
| 图片缺失或加载失败 | 中性占位 | 仍可识别标题并进入详情 |
| 合并预览等待、失败或过期 | 禁止确认 | 等待有效校验或重新预览 |
| 系统打开失败 | 错误反馈，多版本选择可保留 | 重试系统打开 |

任务中心按时间、目录或来源、状态展示日志。自动任务进入待处理状态不等于本地资源不可浏览；本地收录数与资料匹配、待核对、失败数分别呈现，避免将索引成功表达成资料全部获取成功。

## 7. 成人资源的可见性与隔离

```mermaid
flowchart TD
    Setting[设置：媒体类型] --> Save{保存成人类型开关}
    Save -->|启用| Grant[应用建立后台可撤销会话授权]
    Grant --> Type[统一界面出现成人资源类型]
    Type --> Query[独立查询、缓存和权限校验]
    Query --> Pages[同一媒体库、详情、目录与工具结构]
    Save -->|关闭| Revoke[撤销授权并清理前端查询缓存]
    Revoke --> Hidden[隐藏类型，停止相关访问]
    Public[普通全部列表与搜索] --> Ordinary[仅返回普通媒体]
```

前端不增加成人中心、解锁页、锁定按钮或独立目录表单；可见性通过设置控制。统一入口不合并后台数据边界，成人资源不进入普通“全部”列表或普通搜索结果。

## 8. UI/UX 结构与视觉规则

| 层级 | 当前结构 / 仓库约束 |
| --- | --- |
| 窗口框架 | 固定侧栏，右侧内容独立滚动；侧栏底部保留 NFO 和设置 |
| 顶部工具栏 | 搜索、主题切换，以及宿主支持的窗口控件；媒体库内复用同一搜索状态 |
| 媒体库 | 标题与添加入口 → 后台活动 / 恢复状态 → 紧凑筛选 → 海报墙或列表 → 分页 |
| 媒体详情 | 媒体信息与主要动作优先；文件、人员、来源、健康状态等按内容分组 |
| 目录表单 | 系统选择器优先，名称与类型可见，高级路径与排除配置折叠 |
| 合并工作台 | 左侧选择与来源，右侧结果与差异，确认区给出实际后果 |
| 状态呈现 | Badge 表示状态；按钮执行动作。空、加载、错误、缺图分别处理 |
| 文案 | 用分组、对齐、间距表达结构；保留字段标签、真实元数据、错误与操作后果，删除重复介绍 |
| 控件与语言 | 复用仓库 shadcn 共享层，中文 / 英文使用现有 i18next |
| 视觉基调 | `design.md` 要求近黑默认主题、可保存的浅色主题、克制紫色强调、本地字体 |
| 响应与无障碍 | 规范要求窄屏侧栏收缩、可见键盘焦点、对话框焦点管理、减少动态效果、表格局部横滚 |

设计参考：侧栏约 228px，工具栏约 68–76px，海报比例 2:3，常用间距 4–32px，动画约 150–250ms。这些是响应式设计参考，不是固定坐标。

**当前页面与历史设计描述的区别：**默认媒体库是筛选加海报墙，不能将 `design.md` 中保留的 Hero、横卡和最近添加参考构图理解为当前首页已有的全部模块。详情页有专门媒体展示区域；本次未运行应用核对其最终视觉效果。

## 9. 整理依据与验证范围

- 产品边界与视觉规则：[AGENTS.md](../AGENTS.md)、[design.md](../design.md)。
- 路由与应用框架：[app.tsx](../apps/desktop/src/app.tsx)、[app-shell.tsx](../apps/desktop/src/components/shell/app-shell.tsx)、[library-navigation.tsx](../apps/desktop/src/components/shell/library-navigation.tsx)。
- 浏览、详情与目录：[browse.tsx](../apps/desktop/src/features/library/browse.tsx)、[library-filters.tsx](../apps/desktop/src/features/library/library-filters.tsx)、[detail.tsx](../apps/desktop/src/features/library/detail.tsx)、[video-detail.tsx](../apps/desktop/src/features/library/video-detail.tsx)、[use-root-editor.ts](../apps/desktop/src/features/library/use-root-editor.ts)。
- 自动更新：[automatic.rs](../crates/infrastructure/src/scan_service/automatic.rs)；监听与 NFO 策略同时参照根产品约束和相关实现。
- 核对与输出：[scraper/page.tsx](../apps/desktop/src/features/scraper/page.tsx)、[merge-workspace.tsx](../apps/desktop/src/features/scraper/merge-workspace.tsx)。
- 工具与其他入口：[tools/page.tsx](../apps/desktop/src/features/tools/page.tsx)、[music/page.tsx](../apps/desktop/src/features/music/page.tsx)、[nfo/page.tsx](../apps/desktop/src/features/nfo/page.tsx)、[settings/page.tsx](../apps/desktop/src/features/settings/page.tsx)、[tasks.tsx](../apps/desktop/src/features/library/tasks.tsx)。

本次仅新增文档，检查 Markdown 结构和本地引用；未运行桌面应用、端到端流程、Mermaid 渲染器或 `pnpm check`。本文是当前代码与产品约束的流程整理，不作为实机验收记录。
