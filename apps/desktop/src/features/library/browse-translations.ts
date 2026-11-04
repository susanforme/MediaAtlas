import i18n from "../../i18n";

const values = {
  "zh-CN": {
    libraryFilters: "筛选",
    libraryFilteredBy: "高级筛选",
    libraryPosterOpen: "查看详情：{{title}}",
  },
  "en-US": {
    libraryFilters: "Filters",
    libraryFilteredBy: "Advanced filters",
    libraryPosterOpen: "Details: {{title}}",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
