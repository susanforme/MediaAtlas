import type { MediaDetail } from "../../api/library";
import type { MergePreview, RemoteField } from "../../api/scraper";

export type FieldPreset = "all" | "none" | "missing";

function missing(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    (typeof value === "string" && value.trim() === "") ||
    (Array.isArray(value) && value.length === 0)
  );
}

export function selectScraperFields(
  preset: FieldPreset,
  remoteFields: RemoteField[],
  target?: MediaDetail,
  preview?: MergePreview,
): string[] {
  if (preset === "none" || (preset === "missing" && !target)) return [];
  return [...new Set(remoteFields.map((field) => field.key))].filter((key) => {
    const local =
      target?.metadata_values.filter((field) => field.field_key === key) ?? [];
    const diffs = preview?.fields.filter((field) => field.key === key) ?? [];
    if (
      local.some((field) => field.is_locked || field.is_user_modified) ||
      diffs.some(
        (field) =>
          field.protected ||
          field.old?.is_locked ||
          field.old?.is_user_modified,
      )
    )
      return false;
    if (preset === "all") return true;
    const canonical =
      key === "title"
        ? target?.media.title
        : key === "original_title"
          ? target?.media.original_title
          : key === "year"
            ? target?.media.year
            : undefined;
    return (
      missing(canonical) &&
      local.every((field) => missing(field.value)) &&
      diffs.every((field) => !field.old || missing(field.old.value))
    );
  });
}
