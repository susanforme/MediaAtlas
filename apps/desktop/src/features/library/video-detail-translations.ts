import i18n from "../../i18n";

const values = {
  "zh-CN": {
    videoDetailStatus: "资料与媒体状态",
    videoDetailPoster: "海报",
    videoDetailOverview: "简介",
    videoDetailCast: "演职人员",
    videoDetailDirector: "导演",
    videoDetailPremiere: "上映 / 首播",
    videoDetailRuntime: "{{count}} 分钟",
    videoDetailSeasons: "季度与剧集",
    videoDetailSeason: "第 {{number}} 季",
    videoDetailSourceEpisodes: "来源收录 {{count}} 集",
    videoDetailEpisode: "第 {{number}} 集",
    videoDetailNoSeasons: "尚未获取季度与剧集资料",
    videoDetailNoEpisodes: "来源尚未收录本季剧集",
    videoDetailFiles: "本地文件",
    videoDetailAvailable: "已缓存",
    videoDetailMissing: "未获取",
    videoDetailFields: "{{count}} 个字段",
    videoDetailExpand: "展开",
    videoDetailCollapse: "收起",
  },
  "en-US": {
    videoDetailStatus: "Metadata and media status",
    videoDetailPoster: "Poster",
    videoDetailOverview: "Overview",
    videoDetailCast: "Cast and crew",
    videoDetailDirector: "Director",
    videoDetailPremiere: "Released / first aired",
    videoDetailRuntime: "{{count}} min",
    videoDetailSeasons: "Seasons and episodes",
    videoDetailSeason: "Season {{number}}",
    videoDetailSourceEpisodes: "{{count}} episodes in source",
    videoDetailEpisode: "Episode {{number}}",
    videoDetailNoSeasons: "Season and episode details have not been fetched",
    videoDetailNoEpisodes: "The source has no episode details for this season",
    videoDetailFiles: "Local files",
    videoDetailAvailable: "Cached",
    videoDetailMissing: "Not fetched",
    videoDetailFields: "{{count}} fields",
    videoDetailExpand: "Show more",
    videoDetailCollapse: "Show less",
  },
};

for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
