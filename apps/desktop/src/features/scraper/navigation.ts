import i18n from "../../i18n";

// Keep navigation translated before the scraper workspace is loaded on demand.
i18n.addResourceBundle(
  "zh-CN",
  "translation",
  { scraper: "刮削工作台" },
  true,
  true,
);
i18n.addResourceBundle(
  "en-US",
  "translation",
  { scraper: "Scraper" },
  true,
  true,
);
