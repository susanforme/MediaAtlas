import i18n from "../../i18n";

const values = {
  "zh-CN": {
    libraryFilters: "筛选",
    libraryFilteredBy: "高级筛选",
    libraryPosterOpen: "查看详情：{{title}}",
    libraryQuickFilters: "按处理状态筛选",
    libraryQuickAll: "全部",
    libraryQuickIncomplete: "资料不全",
    libraryQuickReview: "待核对",
    libraryQuickUnconfigured: "来源未配置",
    libraryQuickFailed: "资料失败",
    libraryQuickMissingPoster: "缺海报",
  },
  "en-US": {
    libraryFilters: "Filters",
    libraryFilteredBy: "Advanced filters",
    libraryPosterOpen: "Details: {{title}}",
    libraryQuickFilters: "Filter by review status",
    libraryQuickAll: "All",
    libraryQuickIncomplete: "Missing details",
    libraryQuickReview: "Needs review",
    libraryQuickUnconfigured: "Source not configured",
    libraryQuickFailed: "Metadata failed",
    libraryQuickMissingPoster: "Missing poster",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
