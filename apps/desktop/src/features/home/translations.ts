import i18n from "../../i18n";

const translations = {
  "zh-CN": {
    homeFeatured: "精选媒体",
    homeRecent: "最近添加",
    homeExplore: "浏览媒体",
    homeWelcome: "你的媒体，由此展开。",
    homeReadyToScan: "目录已就绪，开始建立媒体库。",
    homeScanBeforeBrowse: "扫描目录并确认识别结果，媒体就会显示在这里。",
    homeReviewTasks: "查看扫描任务",
    homeNewLibrary: "添加媒体目录，建立你的本地收藏。",
    homeHealth: "媒体库状态",
    homeOpenDetails: "查看详情",
  },
  "en-US": {
    homeFeatured: "In your library",
    homeRecent: "Recently added",
    homeExplore: "Explore your media",
    homeWelcome: "Your media. A world to explore.",
    homeReadyToScan: "Your directories are ready.",
    homeScanBeforeBrowse:
      "Scan your directories and review the matches to add media to your library.",
    homeReviewTasks: "Review scan tasks",
    homeNewLibrary: "Add a media directory to build your local collection.",
    homeHealth: "Library status",
    homeOpenDetails: "View details",
  },
};
for (const [locale, values] of Object.entries(translations))
  i18n.addResourceBundle(locale, "translation", values, true, true);
