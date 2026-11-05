import i18n from "../../i18n";

const values = {
  "zh-CN": {
    libraryPosterOpen: "查看详情：{{title}}",
  },
  "en-US": {
    libraryPosterOpen: "Details: {{title}}",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
