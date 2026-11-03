import i18n from "../../i18n";
export const authTranslations = {
  "en-US": {
    providerAuthTitle: "Provider authentication",
    providerAuthAffiliate: "DMM affiliate API",
    providerAuthWikidata: "Wikidata (adult supplement)",
    providerAuthAffiliateHelp:
      "Enter the approved DMM API ID and affiliate ID. Both are stored together in the selected secure store; the backend adds them only to protected original-origin requests.",
    providerAuthApiId: "DMM API ID",
    providerAuthAffiliateId: "DMM affiliate ID",
    providerAuthCleanupPending:
      "Authentication saved; old credentials are awaiting secure cleanup. Refresh the status to retry.",
    providerAuthCleanupDisabled:
      "Authentication is disabled; old credentials are awaiting secure cleanup. Refresh the status to retry.",
    providerAuthRefresh: "Refresh authentication status",
    providerAuthHelp:
      "Save credentials here, then enable the provider separately in Providers. Saving does not test live account authorization or start a request.",
    providerAuthProvider: "Authentication provider",
    providerAuthMethod: "Authentication method",
    providerAuthStorage: "Credential storage",
    providerAuthBearer: "TMDB v4 Read Access Token (Bearer)",
    providerAuthApiKey: "TMDB v3 API key",
    providerAuthBearerHelp:
      "Enter the API Read Access Token from your TMDB API settings. It is sent as a Bearer credential by the protected backend.",
    providerAuthApiKeyHelp:
      "Enter your TMDB v3 API key. The protected backend adds it only to requests for the original TMDB origin and removes it from diagnostics.",
    providerAuthSystem: "System credential store (persistent)",
    providerAuthSession: "This session only (cleared on exit)",
    providerAuthSystemHelp:
      "Uses the operating system credential store. A storage failure is reported; credentials are never silently saved as plaintext or switched to session storage.",
    providerAuthSessionHelp:
      "You explicitly selected temporary memory storage. The credential disappears when the application exits.",
    providerAuthSecret: "Credential",
    providerAuthNoEcho:
      "Saved credentials are never displayed. Enter a new value only to replace the credential.",
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
    providerAuthAffiliateHelp:
      "请填写已批准的 DMM API ID 与联盟 ID。两字段共同保存在所选安全存储中，后端仅向受保护的原始站点请求添加。",
    providerAuthApiId: "DMM API ID",
    providerAuthAffiliateId: "DMM 联盟 ID",
    providerAuthCleanupPending:
      "认证已保存；旧凭据等待安全清理，可刷新状态重试。",
    providerAuthCleanupDisabled:
      "认证已停用；旧凭据等待安全清理，可刷新状态重试。",
    providerAuthRefresh: "刷新认证状态",
    providerAuthHelp:
      "在此保存凭据后，请另行在来源页面启用对应来源。保存不会验证实时账户授权或发起站点请求。",
    providerAuthProvider: "认证来源",
    providerAuthMethod: "认证方式",
    providerAuthStorage: "凭据存储方式",
    providerAuthBearer: "TMDB v4 Read Access Token（Bearer）",
    providerAuthApiKey: "TMDB v3 API Key",
    providerAuthBearerHelp:
      "请填写 TMDB API 设置中的 API Read Access Token，由受控后端作为 Bearer 凭据发送。",
    providerAuthApiKeyHelp:
      "请填写 TMDB v3 API Key，受控后端仅向原始 TMDB 站点请求添加此密钥，诊断记录不会显示密钥。",
    providerAuthSystem: "系统凭据存储（持久保存）",
    providerAuthSession: "仅本次会话（退出后失效）",
    providerAuthSystemHelp:
      "使用操作系统凭据存储。存储失败会明确提示，不会自动以明文保存或切换为会话存储。",
    providerAuthSessionHelp: "你已明确选择临时内存存储，退出应用后凭据失效。",
    providerAuthSecret: "凭据",
    providerAuthNoEcho: "已保存凭据不会回显，仅在需要替换时填写新值。",
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
