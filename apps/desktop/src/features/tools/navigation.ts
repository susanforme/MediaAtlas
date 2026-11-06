import i18n from "../../i18n";

// Navigation is available before the tools route and its workflow strings load.
i18n.addResourceBundle(
  "en-US",
  "translation",
  { videoTools: "Media tools", toolsLocal: "Local media", toolsScraper: "Metadata lookup", toolsHealth: "Checks and monitoring", toolsSections: "Media tools navigation", collapseTools: "Collapse media tools", expandTools: "Expand media tools" },
  true,
  true,
);
i18n.addResourceBundle(
  "zh-CN",
  "translation",
  { videoTools: "媒体工具", toolsLocal: "本地资源", toolsScraper: "资料刮削", toolsHealth: "检查与监控", toolsSections: "媒体工具导航", collapseTools: "收起媒体工具", expandTools: "展开媒体工具" },
  true,
  true,
);
