import type { MediaDetail, MetadataValue } from "../../api/library";
import type {
  ArtworkCandidate,
  MergePreview,
  RemoteField,
  VideoMetadata,
} from "../../api/scraper";

export type MergeFieldResult =
  | "added"
  | "overwritten"
  | "removed"
  | "unchanged"
  | "skipped";

export interface MergePreviewRow {
  key: string;
  language: string | null;
  old: MetadataValue | null;
  new: RemoteField;
  selected: boolean;
  eligible: boolean;
  protected: boolean;
  mandatory: boolean;
  result: MergeFieldResult;
  value: unknown;
}

function empty(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    (typeof value === "string" && value.trim() === "") ||
    (Array.isArray(value) && value.length === 0)
  );
}

function locale(value: string | null): string | null | undefined {
  if (value === null) return null;
  // Match the domain's ASCII locale key; do not infer aliases or fallbacks.
  return /^(?:[a-zA-Z0-9]{1,8})(?:-[a-zA-Z0-9]{1,8})*$/.test(value)
    ? value.toLowerCase()
    : undefined;
}

function equal(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right))
    return (
      left.length === right.length &&
      left.every((value, index) => equal(value, right[index]))
    );
  if (
    left !== null &&
    right !== null &&
    typeof left === "object" &&
    typeof right === "object" &&
    !Array.isArray(left) &&
    !Array.isArray(right)
  ) {
    const a = left as Record<string, unknown>;
    const b = right as Record<string, unknown>;
    return (
      Object.keys(a).length === Object.keys(b).length &&
      Object.keys(a).every(
        (key) => Object.hasOwn(b, key) && equal(a[key], b[key]),
      )
    );
  }
  return false;
}

function canonical(target: MediaDetail | undefined, key: string): unknown {
  if (key === "title") return target?.media.title;
  if (key === "original_title") return target?.media.original_title;
  if (key === "year") return target?.media.year;
  return undefined;
}

/** Local visual projection only. A fresh server preview is required to confirm. */
export function buildMergePreview({
  metadata,
  target,
  selectedFields,
  selectedArtwork,
  onlyMissing,
  preview,
}: {
  metadata: VideoMetadata;
  target?: MediaDetail;
  selectedFields: string[];
  selectedArtwork: number[];
  onlyMissing: boolean;
  preview?: MergePreview;
}) {
  const local = target?.metadata_values ?? [];
  const selected = new Set(selectedFields);
  const authoritative =
    preview?.media_item_id === target?.media.id ? preview : undefined;
  const rows: MergePreviewRow[] = metadata.data.fields.map((field) => {
    const mandatory =
      ["tvmaze", "wikipedia.html", "bangumi", "douban", "omdb"].includes(metadata.data.identity.provider_id) &&
      ["attribution", "source_license"].includes(field.key);
    const language = locale(field.language);
    const existing = local.filter((value) => value.field_key === field.key);
    const proof = authoritative?.fields.find(
      (diff) =>
        diff.key === field.key &&
        locale(diff.language) === language &&
        equal(diff.new.value, field.value),
    );
    const old = proof
      ? proof.old
      : (existing.find((value) => locale(value.locale) === language) ?? null);
    // A conservative whole-key rule also protects independently edited languages,
    // matching the field presets. The server remains the final authority.
    const protectedField =
      existing.some((value) => value.is_locked || value.is_user_modified) ||
      Boolean(proof?.protected || old?.is_locked || old?.is_user_modified);
    const duplicateKey =
      metadata.data.fields.filter((value) => value.key === field.key).length !==
      1;
    const eligible =
      language !== undefined &&
      !duplicateKey &&
      !protectedField &&
      !empty(field.value) &&
      (mandatory ||
        !onlyMissing ||
        (Boolean(target) &&
          empty(canonical(target, field.key)) &&
          existing.every((value) => empty(value.value)) &&
          (!old || empty(old.value))));
    const apply = (mandatory || selected.has(field.key)) && eligible;
    const result: MergeFieldResult = !apply
      ? "skipped"
      : old && equal(old.value, field.value)
        ? "unchanged"
        : !old || empty(old.value)
          ? "added"
          : "overwritten";
    return {
      key: field.key,
      language: field.language,
      old,
      new: field,
      selected: apply,
      eligible,
      protected: protectedField,
      mandatory,
      result,
      value: apply ? field.value : (old?.value ?? canonical(target, field.key)),
    };
  });
  for (const diff of authoritative?.fields ?? []) {
    if (
      diff.new.value !== null || !diff.selected || diff.protected || !diff.old ||
      locale(diff.language) === undefined || diff.new.key !== diff.key ||
      diff.old.field_key !== diff.key || locale(diff.new.language) !== locale(diff.language) ||
      locale(diff.old.locale) !== locale(diff.language) ||
      diff.old.is_locked || diff.old.is_user_modified ||
      local.some((value) => value.field_key === diff.key && (value.is_locked || value.is_user_modified))
    ) continue;
    const index = rows.findIndex((row) => row.key === diff.key && locale(row.language) === locale(diff.language));
    if (index !== -1 && rows[index].selected) continue;
    const removal: MergePreviewRow = {
      key: diff.key, language: diff.language, old: diff.old, new: diff.new,
      selected: true, eligible: false, protected: false, mandatory: true,
      result: "removed", value: null,
    };
    if (index === -1) rows.push(removal);
    else rows[index] = removal;
  }
  const effectiveFields = rows
    .filter((row) => row.selected && row.result !== "removed")
    .map((row) => row.key);
  const effectiveArtwork = [...new Set(selectedArtwork)].filter((index) => {
    const art = metadata.data.artwork[index];
    return (
      Number.isSafeInteger(index) &&
      index >= 0 &&
      art !== undefined &&
      (!onlyMissing ||
        (Boolean(target) &&
          !target?.artworks.some((value) => value.kind === art.kind) &&
          !(target?.poster_uri && ["poster", "cover"].includes(art.kind))))
    );
  });
  const artwork: ArtworkCandidate[] = effectiveArtwork.map(
    (index) => metadata.data.artwork[index],
  );
  const fields: RemoteField[] = local.map((value) => ({
    key: value.field_key,
    language: value.locale,
    value: value.value,
  }));
  for (const row of rows.filter((row) => row.selected)) {
    const index = fields.findIndex(
      (field) =>
        field.key === row.key &&
        locale(field.language) === locale(row.language),
    );
    if (row.result === "removed") {
      if (index !== -1) fields.splice(index, 1);
    } else if (index === -1) fields.push(row.new);
    else fields[index] = row.new;
  }
  const media = {
    title: target?.media.title ?? metadata.data.identity.title,
    original_title:
      target?.media.original_title ??
      (target ? null : metadata.data.identity.original_title),
    year: target?.media.year ?? (target ? null : metadata.data.identity.year),
  };
  for (const row of rows.filter((row) => row.selected)) {
    if (row.result === "removed") continue;
    if (row.key === "title" && typeof row.value === "string")
      media.title = row.value;
    if (row.key === "original_title" && typeof row.value === "string")
      media.original_title = row.value;
    if (
      row.key === "year" &&
      typeof row.value === "number" &&
      Number.isSafeInteger(row.value) &&
      row.value >= -2147483648 &&
      row.value <= 2147483647
    )
      media.year = row.value;
  }
  const counts = {
    added: rows.filter((row) => row.result === "added").length,
    overwritten: rows.filter((row) => row.result === "overwritten" || row.result === "removed").length,
    unchanged: rows.filter((row) => row.result === "unchanged").length,
    skipped: rows.filter((row) => row.result === "skipped").length,
    artwork: artwork.length,
    total: 0,
  };
  counts.total = counts.added + counts.overwritten + counts.artwork;
  return {
    rows,
    fields,
    artwork,
    effectiveFields,
    effectiveArtwork,
    counts,
    media,
    mediaId: target?.media.id,
  };
}

/** A server proof must apply exactly the fields and artwork visible to the user. */
export function mergeSelectionMatches(
  preview: MergePreview,
  projection: ReturnType<typeof buildMergePreview>,
): boolean {
  if (preview.media_item_id !== projection.mediaId) return false;
  const actualFields = preview.fields.filter(
    (field) => field.selected && !field.protected,
  );
  const visibleFields = projection.rows.filter((row) => row.selected);
  if (actualFields.length !== visibleFields.length) return false;
  if (
    !actualFields.every((field) =>
      visibleFields.some(
        (row) =>
          field.key === row.key &&
          locale(field.language) === locale(row.language) &&
          equal(field.new.value, row.new.value),
      ),
    ) ||
    !visibleFields.every((row) =>
      actualFields.some(
        (field) =>
          field.key === row.key &&
          locale(field.language) === locale(row.language) &&
          equal(field.new.value, row.new.value),
      ),
    )
  )
    return false;
  const artworkKey = (art: ArtworkCandidate) =>
    JSON.stringify([
      art.kind,
      art.url,
      art.source_provider,
      locale(art.language),
    ]);
  const actualArtwork = preview.artwork.map(artworkKey).sort();
  const visibleArtwork = projection.artwork.map(artworkKey).sort();
  return (
    actualArtwork.length === visibleArtwork.length &&
    actualArtwork.every((art, index) => art === visibleArtwork[index])
  );
}
