import i18n from "../../i18n";

// Navigation must not load the music workspace and all of its workflow strings.
i18n.addResourceBundle(
  "zh-CN",
  "translation",
  { music: "音乐", musicTools: "音乐工具" },
  true,
  true,
);
i18n.addResourceBundle(
  "en-US",
  "translation",
  { music: "Music", musicTools: "Music tools" },
  true,
  true,
);
