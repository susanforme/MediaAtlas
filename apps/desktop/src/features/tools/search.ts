export interface ToolsSearch {
  media?: string;
}

export function validateToolsSearch(
  value: Record<string, unknown>,
): ToolsSearch {
  return typeof value.media === "string" &&
    value.media.length > 0 &&
    value.media.length <= 128
    ? { media: value.media }
    : {};
}
