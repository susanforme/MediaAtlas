import i18n from "i18next";
import { initReactI18next } from "react-i18next";

export const resources = {
  "zh-CN": {
    translation: {
      language: "语言",
      phase: "Phase 0 · 工程初始化",
      title: "MediaAtlas 工作区",
      subtitle: "本地优先的媒体管理，从这里开始。",
      cardTitle: "桌面连接",
      cardDescription: "检查应用与本地核心的连接状态。",
      loading: "正在连接本地核心…",
      connected: "本地核心已连接",
      desktopRequired: "请在桌面应用中打开",
      desktopDescription:
        "当前为网页预览。启动 MediaAtlas 桌面应用后可检查本地核心连接。",
      invalidResponse: "应用与核心版本不兼容，请更新并重启应用。",
      requestFailed: "无法连接本地核心，请重试。",
      retry: "重新检查",
      checking: "检查中…",
      version: "应用版本",
      reference: "诊断编号",
      scope: "当前版本仅提供桌面连接检查，媒体库功能将按阶段加入。",
      local: "本地工作区",
    },
  },
  en: {
    translation: {
      language: "Language",
      phase: "Phase 0 · Bootstrap",
      title: "MediaAtlas workspace",
      subtitle: "Your local-first media workspace starts here.",
      cardTitle: "Desktop connection",
      cardDescription:
        "Check the connection between the app and its local core.",
      loading: "Connecting to the local core…",
      connected: "Local core connected",
      desktopRequired: "Open in the desktop app",
      desktopDescription:
        "This is a web preview. Start the MediaAtlas desktop app to check its local core connection.",
      invalidResponse:
        "The app and core versions are incompatible. Update and restart the app.",
      requestFailed: "Could not connect to the local core. Please try again.",
      retry: "Check again",
      checking: "Checking…",
      version: "App version",
      reference: "Diagnostic reference",
      scope:
        "This version provides a desktop connection check. Media library features will arrive in later phases.",
      local: "Local workspace",
    },
  },
} as const;

void i18n.use(initReactI18next).init({
  resources,
  lng: "zh-CN",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
