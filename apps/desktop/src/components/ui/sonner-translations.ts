import i18n from "../../i18n";

for (const [language, messages] of Object.entries({
  "zh-CN": { notifications: "通知", dismissNotification: "关闭通知" },
  "en-US": {
    notifications: "Notifications",
    dismissNotification: "Dismiss notification",
  },
}))
  i18n.addResourceBundle(language, "translation", messages, true, true);
