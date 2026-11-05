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
export const providerChoices = ["tmdb", "anilist", "tvmaze", "wikipedia.html"];
export const providerNames: Record<string, string> = {
  tmdb: "TMDB",
  anilist: "AniList",
  tvmaze: "TVmaze",
  "wikipedia.html": "Wikipedia",
};

export function scrapeKind(kind: string): "movie" | "tv" | "anime" | null {
  if (kind === "movie" || kind === "anime") return kind;
  return kind === "tv_series" ? "tv" : null;
}
