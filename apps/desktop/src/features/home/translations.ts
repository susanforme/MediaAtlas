import i18n from "../../i18n";

const translations = {
  "zh-CN": {
    homeFeatured: "精选媒体",
    homeRecent: "最近添加",
    homeExplore: "浏览媒体",
    homeWelcome: "你的媒体，由此展开。",
    homeLibraryPreparing: "你的媒体即将在这里呈现。",
    homeAutomaticLibrary: "正在自动查找媒体并补全资料，发现的文件会陆续显示。",
    homeCheckFolders: "尚未发现可用媒体。检查目录中的文件，或查看处理结果。",
    homeReviewTasks: "查看处理状态",
    homeNewLibrary: "选择媒体目录，自动扫描并获取影片资料。",
    homeHealth: "媒体库状态",
    homeOpenDetails: "查看详情",
  },
  "en-US": {
    homeFeatured: "In your library",
    homeRecent: "Recently added",
    homeExplore: "Explore your media",
    homeWelcome: "Your media. A world to explore.",
    homeLibraryPreparing: "Your media belongs here.",
    homeAutomaticLibrary:
      "Finding media and fetching details automatically. Files appear as they are found.",
    homeCheckFolders:
      "No playable media found yet. Check your folders or review the latest processing results.",
    homeReviewTasks: "View processing status",
    homeNewLibrary:
      "Choose a media folder to scan and fetch movie details automatically.",
    homeHealth: "Library status",
    homeOpenDetails: "View details",
  },
};
for (const [locale, values] of Object.entries(translations))
  i18n.addResourceBundle(locale, "translation", values, true, true);
