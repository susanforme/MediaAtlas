import type { MediaCard, MediaDetail } from "../../api/library";

/** The ordinary home never promotes a private card, even if a provider returns one. */
export function ordinaryCards(cards: MediaCard[]): MediaCard[] {
  return cards.filter((card) => card.media.kind !== "adult_video");
}
export function featuredDescription(
  detail: MediaDetail | undefined,
): string | undefined {
  if (!detail || detail.media.kind === "adult_video") return;
  const value = detail.metadata_values.find((field) =>
    ["overview", "plot", "description", "synopsis"].includes(field.field_key),
  )?.value;
  return typeof value === "string" && value.trim() ? value : undefined;
}
export function featuredBackdrop(
  detail: MediaDetail | undefined,
): string | undefined {
  if (!detail || detail.media.kind === "adult_video") return;
  const source = detail.artworks.find((art) =>
    ["backdrop", "fanart"].includes(art.kind),
  )?.source_url;
  // Desktop CSP permits cached image data only; never bypass it with provider URLs.
  return source?.startsWith("data:image/") ? source : undefined;
}
