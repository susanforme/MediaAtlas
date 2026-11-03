export interface ScraperSearch {
  media?: string;
}

export function validateScraperSearch(
  value: Record<string, unknown>,
): ScraperSearch {
  return typeof value.media === "string" &&
    value.media.length > 0 &&
    value.media.length <= 128
    ? { media: value.media }
    : {};
}
