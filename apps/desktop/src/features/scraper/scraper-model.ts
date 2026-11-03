export function value(v: unknown): string {
  return typeof v === "string" ? v : (JSON.stringify(v) ?? "");
}
export const providerChoices = ["tmdb", "anilist", "wikipedia.html"];
