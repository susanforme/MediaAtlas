import i18n from "../../i18n";

const values = {
  "zh-CN": {
    libraryPosterOpen: "查看详情：{{title}}",
    libraryResetSearch: "清除搜索与筛选",
  },
  "en-US": {
    libraryPosterOpen: "Details: {{title}}",
    libraryResetSearch: "Clear search and filters",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
