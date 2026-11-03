export function value(v: unknown): string {
  return typeof v === "string" ? v : (JSON.stringify(v) ?? "");
}
export const providerChoices = ["tmdb", "anilist", "wikipedia.html"];

export function scrapeKind(kind: string): "movie" | "tv" | "anime" | null {
  if (kind === "movie" || kind === "anime") return kind;
  return kind === "tv_series" ? "tv" : null;
}
