import { DesktopError } from "../../api/desktop";
import type { ProviderFailure } from "../../api/providers";
import type { SearchPreview } from "../../api/scraper";

export type RecoveryKind =
  | "credentials"
  | "configuration"
  | "rate_limit"
  | "timeout"
  | "network"
  | "no_results"
  | "access"
  | "cancelled"
  | "desktop"
  | "invalid_response"
  | "unknown";

export interface SearchRecovery {
  kind: RecoveryKind;
  providerId?: string;
}

const failureKinds: Record<ProviderFailure, RecoveryKind> = {
  credentials_unavailable: "credentials",
  disabled: "configuration",
  invalid_configuration: "configuration",
  rate_limited: "rate_limit",
  cooling_down: "rate_limit",
  timeout: "timeout",
  dns_error: "network",
  tls_error: "network",
  network_error: "network",
  http5xx: "network",
  not_found: "no_results",
  blocked: "access",
  captcha: "access",
  cancelled: "cancelled",
  // The wire contract does not distinguish 401 from other HTTP 4xx errors.
  http4xx: "unknown",
  parse_error: "unknown",
  identity_mismatch: "unknown",
  unsafe_target: "unknown",
  response_too_large: "unknown",
};

function failureKind(code: string): RecoveryKind {
  return Object.hasOwn(failureKinds, code)
    ? failureKinds[code as ProviderFailure]
    : "unknown";
}

function errorKind(error: unknown): RecoveryKind {
  if (error instanceof DesktopError) {
    if (error.code === "desktop_required") return "desktop";
    if (error.code === "invalid_response") return "invalid_response";
    return failureKind(error.envelope?.code ?? "");
  }
  // ErrorDto may also be supplied by a caller before DesktopError wrapping.
  if (
    typeof error === "object" &&
    error !== null &&
    "schema_version" in error &&
    error.schema_version === 1 &&
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string" &&
    "correlation_id" in error &&
    typeof error.correlation_id === "string"
  )
    return failureKind(error.code);
  return "unknown";
}

/** Interpret only allowlisted codes, never error messages, paths or HTTP guesses. */
export function searchRecoveries(
  search?: SearchPreview,
  error?: unknown,
): SearchRecovery[] {
  if (error !== undefined && error !== null)
    return [{ kind: errorKind(error) }];
  if (!search) return [];
  const recoveries = search.failures.map(({ failure, provider_id }) => ({
    kind: failureKind(failure),
    providerId: provider_id,
  }));
  if (recoveries.length === 0 && search.candidates.length === 0)
    return [{ kind: "no_results" }];
  return recoveries.filter(
    (recovery, index) =>
      recoveries.findIndex(
        (other) =>
          other.kind === recovery.kind &&
          other.providerId === recovery.providerId,
      ) === index,
  );
}
