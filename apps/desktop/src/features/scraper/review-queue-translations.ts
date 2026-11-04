import i18n from "../../i18n";

const values = {
  "zh-CN": {
    reviewQueues: "已保存的核对队列",
    reviewResume: "继续核对",
    reviewSummary:
      "{{count}} 项 · 已确认 {{confirmed}} 项 · 暂缓 {{deferred}} 项 · 失败 {{failed}} 项",
    reviewState_pending: "待核对",
    reviewState_deferred: "已暂缓",
    reviewState_confirmed: "已确认",
    reviewState_failed: "处理失败",
    reviewDefer: "暂缓此项",
    reviewRetry: "重新核对此项",
    reviewPrevious: "上一项",
    reviewNext: "下一项",
    reviewItemPicker: "当前条目",
    reviewPosition: "第 {{index}} / {{count}} 项",
    reviewRefresh: "刷新队列",
    reviewUnavailable: "此媒体已移除或所在目录不可用。恢复目录后刷新队列。",
    reviewRecheck: "之前的预览已失效，请重新核对。",
    reviewFailed:
      "本项处理失败，重新核对后可再次检索；不会自动确认或写入文件。",
    reviewDeferred: "此项已暂缓，继续时需要重新核对。",
    reviewFinished: "当前队列没有待核对项目。",
  },
  "en-US": {
    reviewQueues: "Saved review queues",
    reviewResume: "Resume review",
    reviewSummary:
      "{{count}} items · {{confirmed}} confirmed · {{deferred}} deferred · {{failed}} failed",
    reviewState_pending: "Pending review",
    reviewState_deferred: "Deferred",
    reviewState_confirmed: "Confirmed",
    reviewState_failed: "Failed",
    reviewDefer: "Defer this item",
    reviewRetry: "Review this item again",
    reviewPrevious: "Previous",
    reviewNext: "Next",
    reviewItemPicker: "Current item",
    reviewPosition: "{{index}} / {{count}}",
    reviewRefresh: "Refresh queue",
    reviewUnavailable:
      "This media was removed or its folder is unavailable. Restore the folder and refresh the queue.",
    reviewRecheck: "The previous preview has expired. Review this item again.",
    reviewFailed:
      "This item failed. Review it again to retry the search; metadata and files are never applied automatically.",
    reviewDeferred: "This item was deferred. Review it again to continue.",
    reviewFinished: "There are no pending items in this queue.",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
