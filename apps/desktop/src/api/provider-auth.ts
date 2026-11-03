import { request } from "./library";
export type AuthMethod = "none" | "bearer" | "api_key";
export type AuthStorage = "system" | "session_only";
export type AuthStatusCode =
  | "not_required"
  | "not_configured"
  | "configured"
  | "credentials_unavailable"
  | "missing_credentials"
  | "cleanup_pending";
export interface AuthStatus {
  provider_id: string;
  supported_methods: AuthMethod[];
  method: AuthMethod;
  storage: AuthStorage;
  configured: boolean;
  status: AuthStatusCode;
}
export interface AuthSaveInput {
  provider_id: string;
  method: AuthMethod;
  storage: AuthStorage;
  secret: string;
}
function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
const methods: AuthMethod[] = ["none", "bearer", "api_key"];
const statuses: AuthStatusCode[] = [
  "not_required",
  "not_configured",
  "configured",
  "credentials_unavailable",
  "missing_credentials",
  "cleanup_pending",
];
export function authStatus(value: unknown): value is AuthStatus {
  return (
    object(value) &&
    typeof value.provider_id === "string" &&
    value.provider_id.length <= 128 &&
    Array.isArray(value.supported_methods) &&
    value.supported_methods.length > 0 &&
    value.supported_methods.length <= methods.length &&
    value.supported_methods.every((method) => methods.includes(method)) &&
    methods.includes(value.method as AuthMethod) &&
    ["system", "session_only"].includes(String(value.storage)) &&
    typeof value.configured === "boolean" &&
    statuses.includes(value.status as AuthStatusCode) &&
    Object.keys(value).every((key) =>
      [
        "provider_id",
        "supported_methods",
        "method",
        "storage",
        "configured",
        "status",
      ].includes(key),
    )
  );
}
function forProvider(providerId: string) {
  return (value: unknown): value is AuthStatus =>
    authStatus(value) && value.provider_id === providerId;
}
export const providerAuthApi = {
  status: (providerId: string) =>
    request(
      "provider_auth_status",
      { provider_id: providerId },
      forProvider(providerId),
    ),
  save: (input: AuthSaveInput) =>
    request("provider_auth_save", input, forProvider(input.provider_id)),
  clear: (providerId: string) =>
    request(
      "provider_auth_clear",
      { provider_id: providerId },
      forProvider(providerId),
    ),
};
