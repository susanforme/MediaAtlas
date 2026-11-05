import type { MediaDetail } from "../../api/library";
import type { MergePreview, VideoMetadata } from "../../api/scraper";

export function identityMayChange(metadata: VideoMetadata, current?: MediaDetail): boolean {
  return metadata.data.identity.external_ids.some(({ namespace }) => {
    const incoming = metadata.data.identity.external_ids.filter((id) => id.namespace === namespace).map((id) => id.value).sort();
    const existing = (current?.external_ids ?? []).filter((id) => id.namespace === namespace).map((id) => id.external_id).sort();
    return JSON.stringify(incoming) !== JSON.stringify(existing);
  });
}

export function changedExternalIds(preview?: MergePreview) {
  return (preview?.external_ids ?? []).filter((id) =>
    JSON.stringify([...id.old].sort()) !== JSON.stringify([...id.new].sort()),
  );
}

export function value(v: unknown): string {
  if (Array.isArray(v)) return v.map(value).filter(Boolean).join("、");
  if (
    v !== null && typeof v === "object" && "name" in v && typeof v.name === "string"
  ) {
    const role = "role" in v && typeof v.role === "string" ? v.role : undefined;
    return role ? `${v.name} (${role})` : v.name;
  }
  return typeof v === "string" ? v : (JSON.stringify(v) ?? "");
}
export const providerChoices = ["tmdb", "omdb", "douban", "anilist", "bangumi", "tvmaze", "wikipedia.html"];
export const providerNames: Record<string, string> = {
  tmdb: "TMDB",
  anilist: "AniList",
  tvmaze: "TVmaze",
  omdb: "OMDb",
  douban: "Douban",
  bangumi: "Bangumi",
  "wikipedia.html": "Wikipedia",
};

export function scrapeKind(kind: string): "movie" | "tv" | "anime" | null {
  if (kind === "movie" || kind === "anime") return kind;
  return kind === "tv_series" ? "tv" : null;
}
