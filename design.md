# MediaAtlas 桌面设计规范

本规范约束后续桌面开发。参考用户提供的 1648 × 928 示意图，仅提取视觉语言；下面的 MediaAtlas 业务约束优先于参考指南中示例品牌、导航、功能和内容。

## 产品与实现边界（必须遵守）

- 品牌保持 **MediaAtlas**。导航仅对应仓库已有业务，不复制 MediaHub、发现、游戏、通知、头像、下载、收藏等示例功能。
- 普通媒体库只读；媒体通过系统关联 App 打开，不添加内嵌播放器或捆绑浏览器引擎。扫描、刮削、组织等写入流程保留既有预览、明确确认与权限边界。
- 成人内容保持独立入口、授权、query/cache 和锁定清理，不出现在普通首页、搜索、推荐或海报墙。保留可配置常显入口。
- Token 由用户手动配置并显式保存；不自动读取运行环境密钥，不在界面、日志、截图或代码中暴露 Token。
- 所有内容和计数必须来自真实业务数据。没有观看进度契约就不显示“继续观看”或进度条；横卡可以呈现真实媒体与元数据。没有真实评分、简介、背景图时省略对应字段；禁止用假数据填充。
- 所有业务交互控件使用本仓库 shadcn 共享层 `components/ui/`。先补缺失组件，再在页面使用；页面不直接写原生 button/input/select/textarea 等交互控件，不使用 `window.confirm` / `window.alert`。语义链接通过共享 Button 的组合入口呈现；保留无障碍 label 和原有受控事件契约。
- 遵守 `AGENTS.md` 与 `apps/desktop/src/AGENTS.md`：薄路由、feature 页面、业务组件、hook 状态边界和纯逻辑分离。文案沿用双语 i18next，删除重复说明。

## 视觉实施与验收

- 电影式近黑界面；克制紫色强调；约 228px 固定侧栏，底部设置；约 68px 悬浮工具栏。不制造无效历史、窗口或通知按钮。
- Hero 无卡片边界，图片与背景渐变融合，左侧可读信息，右侧图像主体。仅使用当前普通媒体数据和已有授权图片；缺图时采用中性占位，不伪造封面。
- 横向媒体卡片约 326 × 130px；最近添加使用 2:3 海报与下方标题。参考尺寸可响应式调整，不使用绝对坐标拼图。
- 主题、公共控件、公共状态样式集中管理；页面样式放所属 feature。支持窄屏侧栏收缩/抽屉、键盘焦点、可读对比度与 `prefers-reduced-motion`。
- 必须重新运行适当检查；检查 1648 × 928 与窄屏、键盘、减少动态效果、加载、空、错误、图片缺失。真实运行截图保存在根 `screenshots/`，注明运行方式与数据限制；测试 fixture 不作为产品真实内容。
- 在 main 分阶段提交，作者/提交者均为 Codex <codex@openai.com>。共享框架与 shadcn、首页与媒体浏览、工具与设置并行协作；共享文件指定唯一负责人。本轮不等待桌面打包，不移动既有 tag。

## 原始视觉参考（仅风格资料）

以下保留用户提供的近似视觉参数。出现 MediaHub 品牌、示例导航、播放/收藏、继续观看、假定评分等内容时，必须按上面的 MediaAtlas 业务边界调整，不能直接实现示例功能。

# MediaHub UI Design Language — AI Implementation Guide

> Reference: user-provided MediaHub desktop screenshot (1648 × 928). This is a visual reconstruction specification, **not** an extraction from source CSS or Figma. All dimensions, colors, and typography are approximate and should be calibrated against the screenshot.

## Objective

Implement a polished desktop media-center UI matching the reference image's **cinematic dark aesthetic**. Prioritize layout fidelity, content hierarchy, image treatment, and interaction consistency over decorative effects. The UI should feel like a premium native desktop application rather than a generic web dashboard.

## Visual identity

- Style: **Cinematic Immersive UI / Premium Media Dashboard / Dark Minimalism**.
- Content-first: posters, stills, and hero artwork dominate the visual hierarchy.
- Frame: near-black chrome, subtly separated fixed sidebar, restrained translucent controls.
- Brand accent: vivid violet, used sparingly for active navigation, primary actions, and progress indicators.
- Media artwork provides ambient orange, blue, or pink lighting; **do not** make those colors global UI tokens.
- Avoid bright borders, excessive glow, heavy neumorphism, and generic dashboard widgets.

## Approximate design tokens

```css
:root {
  --bg-app: #101016;
  --bg-sidebar: #17171f;
  --bg-surface: #25232d;
  --bg-hover: #302d3a;
  --color-primary: #7657f5;
  --color-primary-hover: #896eff;
  --color-primary-muted: #393577;
  --text-primary: #f5f4fa;
  --text-secondary: #a5a3b1;
  --text-muted: #777480;
  --color-rating: #f5bb46;
  --color-progress: #7657f5;
  --overlay-dark: rgba(10, 9, 16, 0.72);
  --surface-glass: rgba(255, 255, 255, 0.08);
  --border-subtle: rgba(255, 255, 255, 0.08);
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 14px;
  --radius-xl: 18px;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;
  --sidebar-width: 228px;
  --toolbar-height: 68px;
  --content-padding: 40px;
  --transition-fast: 150ms ease;
  --transition-normal: 250ms ease;
}
```

Use a clean sans-serif with good Chinese support, e.g. `Inter, "Noto Sans SC", "Microsoft YaHei", sans-serif`. The screenshot does not identify the actual font. Use roughly 14–16px body text, 20–24px section headings, 30–36px hero title, with stronger weights for headings and selected navigation.

## Desktop composition

Target reference viewport: **1648 × 928**.

### 1. Fixed left sidebar

- Width approximately **228px**, full viewport height, dark charcoal background, subtle right divider.
- Brand/logo at upper left: violet icon + white “MediaHub” wordmark.
- Top-level navigation: Home (selected), Media Library (expanded), Discover, Scraper, Download Tasks (count badge), Favorites, Tags; Settings pinned near bottom.
- Expanded library subnavigation: Movies, TV Shows, Anime, Games, Music.
- Navigation rows: around 44–48px high; subdued outline icons, white/light-gray labels.
- Selected row: muted violet rounded rectangle with brighter violet icon; no high-contrast outline.
- Maintain clear indentation for child items, dividers between navigation groups, and a quiet bottom settings region.

### 2. Top toolbar

- Overlays the hero area rather than occupying a visually separate solid header.
- Left: circular back and forward buttons; wide rounded translucent search field with search icon and Chinese placeholder.
- Right: gamepad, notification bell with small unread dot, circular avatar, window controls (minimize, maximize, close).
- Approximate toolbar height **68px**. Controls use translucent dark fills and restrained borders.
- For actual desktop shell integrations, use platform-safe draggable and non-draggable regions.

### 3. Cinematic hero

- Full-width hero artwork behind the content, beginning at the top of the main panel.
- Image subject weighted toward the right; leave readable negative space on the left for text.
- Strong multi-directional overlays: darken left for legibility, fade bottom into app background, and softly blend the top edge.
- Hero information aligned left, starting roughly 68px from the main panel's left edge and around 115px from the top.
- Content: stylized movie logo (if available), Chinese title, star rating, year, type, genres, two-line synopsis, primary/secondary action buttons.
- Primary button: violet “播放” with play icon. Secondary actions: “详情”, “已收藏”, more menu; use translucent gray fills.
- Hero carousel indicators at lower right, with active indicator wider and brighter.
- The image should blend seamlessly into the rest of the page; **no obvious rectangular hero-card boundary**.

### 4. Continue Watching

- Section title “继续观看” with chevron, overlaid close to the hero's lower fade region.
- Horizontal row of approximately four wide landscape cards, with ~14–16px gaps.
- At reference size each card is about **326 × 130px**.
- Cards use `object-fit: cover` artwork, bottom dark gradient, title and remaining-time/subtitle near lower left.
- Thin progress track at bottom, violet completed portion, muted gray remainder.
- Rounded corners about 10px. Avoid excessive shadow or thick border.

### 5. Recently Added

- Section title “最近添加” with chevron, separated vertically from the previous row by about 30px.
- Horizontal row of poster cards; about seven visible at the reference width.
- Approximate poster width **182px** with 14–16px gaps; artwork should preserve its intended poster ratio (typically ~2:3), continuing below the screenshot fold as needed.
- Titles sit below the poster image, not over its center.
- Use diverse cinematic cover art. Do not stretch, blur, or duplicate a single poster across the row.

## Component behavior

- **Navigation:** hover uses subtle lightening; selected state uses muted violet background. Expanded child navigation remains visually subordinate.
- **Buttons:** 150–250ms transitions; slight brightness shift on hover; clear keyboard focus ring; disabled state visibly subdued.
- **Cards:** slight lift or image scale on hover (e.g. 1.02), subtle overlay and optional contextual play control; avoid large motion.
- **Progress:** render actual fractional progress; never use arbitrary equal-length bars.
- **Carousel:** accessible previous/next controls, indicators, and stable text alignment while artwork changes.
- **Search:** usable input with keyboard focus and empty/query states.
- **Responsive:** preserve desktop layout first; at narrower widths, reduce content padding and visible card count; collapse sidebar into a compact icon rail or drawer when necessary. Do not squeeze the 228px sidebar and seven posters into a narrow viewport.
- Respect `prefers-reduced-motion: reduce`.

## Imagery and assets

- Use high-quality, correctly cropped film/game artwork supplied by the project or legally accessible metadata/image providers.
- Preserve each title's identity and use accurate media labels where possible.
- The hero's warm orange light is artwork-specific; the application chrome remains neutral dark.
- Prefer real logo assets for featured media; if unavailable, use tasteful text rather than attempting an inaccurate logo recreation.
- Lazy-load offscreen posters and provide fallbacks for missing images.

## Suggested component structure

```text
MediaHubApp
├── Sidebar
│   ├── Brand
│   ├── NavigationGroup
│   ├── ExpandableLibraryNav
│   ├── DownloadBadge
│   └── SettingsEntry
└── MainContent
    ├── TopToolbar
    │   ├── HistoryControls
    │   ├── SearchInput
    │   ├── UtilityActions
    │   └── WindowControls
    ├── FeaturedHeroCarousel
    │   ├── HeroArtwork
    │   ├── HeroMetadata
    │   ├── HeroActions
    │   └── CarouselIndicators
    ├── ContinueWatchingSection
    │   └── ContinueWatchingCard[]
    └── RecentlyAddedSection
        └── PosterCard[]
```

Build components with reusable props and real data models, not hard-coded absolute coordinates. Use CSS Grid/Flexbox for rows, a fixed or sticky sidebar, and layered positioned backgrounds only where needed for the hero. The hero content and media rows must remain independently maintainable.

## Quality acceptance checklist

- [ ] Overall silhouette closely matches reference at 1648 × 928.
- [ ] Sidebar width, indentation, active state, and pinned settings match.
- [ ] Toolbar visually floats above hero without an opaque horizontal bar.
- [ ] Hero artwork is right-weighted and blends smoothly into dark app background.
- [ ] Hero title, metadata, synopsis, and buttons have correct hierarchy and spacing.
- [ ] Continue Watching shows four landscape cards with progress and gradient overlays.
- [ ] Recently Added displays a dense horizontal poster gallery.
- [ ] Violet is reserved for meaningful interactive accents.
- [ ] Chinese typography is legible and aligned consistently.
- [ ] Hover, focus, loading, empty, and missing-image states are implemented.
- [ ] No stretched artwork, clipping, horizontal page overflow, or excessive glow.

## Instructions to the coding agent

**Implement the supplied screenshot as a working, reusable UI**, using the existing project framework and conventions. Treat this document as the visual specification and the screenshot as the final visual source of truth. Begin with layout and visual hierarchy, then style components, wire interactions, and perform screenshot-based visual QA at the reference viewport. Do not invent unrelated dashboard panels, charts, gradients, or additional navigation. If exact assets or measurements are unavailable, make conservative approximations and clearly isolate them for later replacement.