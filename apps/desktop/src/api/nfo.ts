import { isTauri } from "@tauri-apps/api/core";
import { request } from "./library";

export interface NfoDocument {
  name: string;
  format: "xml" | "text";
  source: string;
  metadata: {
    fields: Record<string, unknown>;
    external_ids: { provider_id: string; namespace: string; value: string }[];
  };
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function document(value: unknown): value is NfoDocument {
  return (
    record(value) &&
    typeof value.name === "string" &&
    typeof value.source === "string" &&
    (value.format === "xml" || value.format === "text") &&
    record(value.metadata) &&
    record(value.metadata.fields) &&
    Array.isArray(value.metadata.external_ids) &&
    value.metadata.external_ids.every(
      (id: unknown) =>
        record(id) &&
        typeof id.provider_id === "string" &&
        typeof id.namespace === "string" &&
        typeof id.value === "string",
    )
  );
}

const optionalDocument = (value: unknown): value is NfoDocument | null =>
  value === null || document(value);

export function isNfoStartup(): Promise<boolean> {
  if (!isTauri()) return Promise.resolve(false);
  return request(
    "nfo_startup_mode",
    {},
    (value): value is boolean => typeof value === "boolean",
  );
}

export const nfoApi = {
  hasOpened: () =>
    request(
      "nfo_has_opened",
      {},
      (value): value is boolean => typeof value === "boolean",
    ),
  takeOpened: () => request("nfo_take_opened", {}, optionalDocument),
  open: (language: string) =>
    request("nfo_open_dialog", { language }, optionalDocument),
};
