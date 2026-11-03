import { invoke, isTauri } from "@tauri-apps/api/core";

export interface AppInfo {
  schema_version: 1;
  app_name: "MediaAtlas";
  app_version: string;
}

export interface IpcErrorEnvelope {
  schema_version: 1;
  code: string;
  message: string;
  correlation_id: string;
}

type DiagnosticFailure =
  | "desktop_required"
  | "invalid_response"
  | "request_failed";

export class DesktopError extends Error {
  constructor(
    readonly code: DiagnosticFailure,
    readonly envelope?: IpcErrorEnvelope,
  ) {
    super(code);
    this.name = "DesktopError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isErrorEnvelope(value: unknown): value is IpcErrorEnvelope {
  return (
    isRecord(value) &&
    value.schema_version === 1 &&
    typeof value.code === "string" &&
    typeof value.message === "string" &&
    typeof value.correlation_id === "string"
  );
}

export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) throw new DesktopError("desktop_required");

  let value: unknown;
  try {
    value = await invoke<unknown>("get_app_info");
  } catch (error: unknown) {
    // Never display raw transport errors: they may contain local paths or secrets.
    throw new DesktopError(
      "request_failed",
      isErrorEnvelope(error) ? error : undefined,
    );
  }

  if (
    !isRecord(value) ||
    value.schema_version !== 1 ||
    value.app_name !== "MediaAtlas" ||
    typeof value.app_version !== "string" ||
    value.app_version.length === 0
  ) {
    throw new DesktopError("invalid_response");
  }

  return {
    schema_version: 1,
    app_name: "MediaAtlas",
    app_version: value.app_version,
  };
}
