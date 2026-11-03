import i18n from "../../i18n";
export const authTranslations = {
  "en-US": {
    providerAuthTitle: "Provider authentication",
    providerAuthAffiliate: "DMM affiliate API",
    providerAuthWikidata: "Wikidata (adult supplement)",

    providerAuthApiId: "DMM API ID",
    providerAuthAffiliateId: "DMM affiliate ID",
    providerAuthCleanupPending:
      "Authentication saved; old credentials are awaiting secure cleanup. Refresh the status to retry.",
    providerAuthCleanupDisabled:
      "Authentication is disabled; old credentials are awaiting secure cleanup. Refresh the status to retry.",
    providerAuthRefresh: "Refresh authentication status",

    providerAuthProvider: "Authentication provider",
    providerAuthMethod: "Authentication method",
    providerAuthStorage: "Credential storage",
    providerAuthBearer: "TMDB v4 Read Access Token (Bearer)",
    providerAuthApiKey: "TMDB v3 API key",

    providerAuthSystem: "System credential store (persistent)",
    providerAuthSession: "This session only (cleared on exit)",

    providerAuthSecret: "Credential",
    providerAuthNoEcho: "Enter a new value to replace the saved credential.",
    providerAuthSave: "Save authentication",
    providerAuthClear: "Delete saved authentication",
    providerAuthSaved:
      "Authentication saved. Provider enablement is unchanged.",
    providerAuthCleared: "Saved authentication deleted.",
    providerAuthSaveFailed:
      "Authentication was not saved. Check the error and explicitly choose session-only storage if the system store is unavailable.",
    providerAuthClearFailed:
      "Could not delete authentication. Check the error and retry.",
    providerAuthConfigured: "Credential configured",
    providerAuthMissing: "No credential configured",
    providerAuthPublic:
      "This provider uses public read-only access and requires no credential.",
    providerAuthInvalid:
      "Enter a non-empty credential without whitespace or control characters.",
    providerAuthStorageStatus: "Current storage: {{storage}}",
    providerAuthSaving: "Saving authentication…",
    providerAuthDeleting: "Deleting authentication…",
    providerAuthLoading: "Reading authentication status…",
  },
  "zh-CN": {
    providerAuthTitle: "来源认证",
    providerAuthAffiliate: "DMM 联盟 API",
    providerAuthWikidata: "Wikidata（成人补充来源）",

    providerAuthApiId: "DMM API ID",
    providerAuthAffiliateId: "DMM 联盟 ID",
    providerAuthCleanupPending:
      "认证已保存；旧凭据等待安全清理，可刷新状态重试。",
    providerAuthCleanupDisabled:
      "认证已停用；旧凭据等待安全清理，可刷新状态重试。",
    providerAuthRefresh: "刷新认证状态",

    providerAuthProvider: "认证来源",
    providerAuthMethod: "认证方式",
    providerAuthStorage: "凭据存储方式",
    providerAuthBearer: "TMDB v4 Read Access Token（Bearer）",
    providerAuthApiKey: "TMDB v3 API Key",

    providerAuthSystem: "系统凭据存储（持久保存）",
    providerAuthSession: "仅本次会话（退出后失效）",

    providerAuthSecret: "凭据",
    providerAuthNoEcho: "填写新值以替换已保存的凭据。",
    providerAuthSave: "保存认证",
    providerAuthClear: "删除已保存认证",
    providerAuthSaved: "认证已保存，来源启用状态保持不变。",
    providerAuthCleared: "已删除保存的认证。",
    providerAuthSaveFailed:
      "认证未保存，请查看错误；若系统存储不可用，可明确选择仅本次会话存储。",
    providerAuthClearFailed: "无法删除认证，请查看错误后重试。",
    providerAuthConfigured: "已配置凭据",
    providerAuthMissing: "尚未配置凭据",
    providerAuthPublic: "此来源使用公开只读访问，无需填写凭据。",
    providerAuthInvalid: "请填写不含空白或控制字符的非空凭据。",
    providerAuthStorageStatus: "当前存储：{{storage}}",
    providerAuthSaving: "正在保存认证…",
    providerAuthDeleting: "正在删除认证…",
    providerAuthLoading: "正在读取认证状态…",
  },
} as const;
for (const language of ["zh-CN", "en-US"] as const)
  i18n.addResourceBundle(
    language,
    "translation",
    authTranslations[language],
    true,
    true,
  );
