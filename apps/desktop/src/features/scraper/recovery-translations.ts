import i18n from "../../i18n";

const values = {
  "zh-CN": {
    scraperRecovery_credentials: "来源凭据不可用，请手动配置并保存。",
    scraperRecovery_configuration: "来源未启用或配置无效，请检查配置。",
    scraperRecovery_rate_limit: "来源暂时限制请求，请稍后重试。",
    scraperRecovery_timeout: "来源请求超时，可以重新搜索当前项。",
    scraperRecovery_network: "暂时无法连接来源，请检查网络后重试。",
    scraperRecovery_no_results:
      "没有找到匹配结果，可以修改标题、年份或来源后重新搜索。",
    scraperRecovery_access: "来源限制了访问，请检查来源状态或选择其他来源。",
    scraperRecovery_cancelled: "来源请求已取消，可以重新搜索当前项。",
    scraperRecovery_desktop: "请在桌面应用中打开。",
    scraperRecovery_invalid_response: "来源响应无法读取，可以重新搜索当前项。",
    scraperRecovery_unknown: "未能完成来源请求，可以修改查询或重新搜索当前项。",
    scraperRecoveryEdit: "修改查询",
    scraperRecoveryConfigure: "配置来源并返回",
    scraperRecoveryRetry: "重新搜索当前项",
  },
  "en-US": {
    scraperRecovery_credentials:
      "Source credentials are unavailable. Configure and save them manually.",
    scraperRecovery_configuration:
      "The source is disabled or misconfigured. Check its configuration.",
    scraperRecovery_rate_limit:
      "The source is limiting requests. Try again later.",
    scraperRecovery_timeout:
      "The source request timed out. You can search this item again.",
    scraperRecovery_network:
      "The source could not be reached. Check your connection and try again.",
    scraperRecovery_no_results:
      "No matches found. Change the title, year or source and search again.",
    scraperRecovery_access:
      "The source restricted access. Check its status or choose another source.",
    scraperRecovery_cancelled:
      "The source request was cancelled. You can search this item again.",
    scraperRecovery_desktop: "Open this in the desktop application.",
    scraperRecovery_invalid_response:
      "The source response could not be read. You can search this item again.",
    scraperRecovery_unknown:
      "The source request failed. Edit the query or search this item again.",
    scraperRecoveryEdit: "Edit query",
    scraperRecoveryConfigure: "Configure source and return",
    scraperRecoveryRetry: "Search this item again",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
