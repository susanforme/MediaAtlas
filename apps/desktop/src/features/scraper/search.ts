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

export interface ScraperReturnSearch extends ScraperSearch {
  from?: "scraper";
}

export function validateScraperReturnSearch(
  value: Record<string, unknown>,
): ScraperReturnSearch {
  return value.from === "scraper"
    ? { from: "scraper", ...validateScraperSearch(value) }
    : {};
}
