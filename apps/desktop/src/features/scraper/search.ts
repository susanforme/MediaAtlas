export const MAX_SCRAPE_QUEUE = 32;

export interface ScraperSearch {
  media?: string;
  queue?: string[];
}

function validMediaId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 128 &&
    value.trim().length > 0 &&
    [...value].every(
      (character) =>
        character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127,
    )
  );
}

export function validateScraperSearch(
  value: Record<string, unknown>,
): ScraperSearch {
  const media = validMediaId(value.media) ? value.media : undefined;
  const parsed =
    Array.isArray(value.queue) &&
    value.queue.length > 0 &&
    value.queue.length <= MAX_SCRAPE_QUEUE &&
    Array.from(value.queue).every(validMediaId)
      ? [...new Set(value.queue)]
      : undefined;
  const queue =
    parsed && (!media || parsed.includes(media)) ? parsed : undefined;
  const selected = media ?? queue?.[0];
  return {
    ...(selected ? { media: selected } : {}),
    ...(queue ? { queue } : {}),
  };
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
