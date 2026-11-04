import i18n from "../../i18n";

const resources = {
  "zh-CN": {
    windowControls: "窗口控制",
    windowMinimize: "最小化窗口",
    windowMaximize: "最大化窗口",
    windowRestore: "还原窗口",
    windowClose: "关闭窗口",
    windowOperationFailed: "窗口操作未完成，请重试。",
    windowRetry: "重试窗口控制",
  },
  "en-US": {
    windowControls: "Window controls",
    windowMinimize: "Minimize window",
    windowMaximize: "Maximize window",
    windowRestore: "Restore window",
    windowClose: "Close window",
    windowOperationFailed: "The window operation did not finish. Try again.",
    windowRetry: "Retry window controls",
  },
} as const;

for (const language of ["zh-CN", "en-US"] as const)
  i18n.addResourceBundle(
    language,
    "translation",
    resources[language],
    true,
    true,
  );
