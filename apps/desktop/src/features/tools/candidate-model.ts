import type { MediaCard } from "../../api/library";
import { MAX_SCRAPE_QUEUE } from "../scraper/search";
import { TOOLS_KINDS } from "./search";

function hasCandidateId(card: MediaCard): boolean {
  const id = card.media.id;
  return (
    typeof id === "string" &&
    id.length > 0 &&
    id.length <= 128 &&
    !/\s/.test(id) &&
    [...id].every(
      (character) =>
        character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127,
    )
  );
}
export function isSelectableCandidate(card: MediaCard): boolean {
  return (
    TOOLS_KINDS.some((kind) => kind === card.media.kind) && hasCandidateId(card)
  );
}
export function ordinaryCandidates(cards: MediaCard[]): MediaCard[] {
  const privateIds = new Set(
    cards
      .filter((card) => card.media.kind === "adult_video")
      .map((card) => card.media.id),
  );
  const seen = new Set<string>();
  return cards.filter((card) => {
    const id = card.media.id;
    if (!hasCandidateId(card) || privateIds.has(id) || seen.has(id))
      return false;
    seen.add(id);
    return true;
  });
}
/** The shared table never trusts a response to cross its selected resource type. */
export function adultCandidates(cards: MediaCard[]): MediaCard[] {
  const seen = new Set<string>();
  return cards.filter((card) => {
    if (
      card.media.kind !== "adult_video" ||
      !hasCandidateId(card) ||
      seen.has(card.media.id)
    )
      return false;
    seen.add(card.media.id);
    return true;
  });
}
export type CandidateSelectionResult = {
  cards: MediaCard[];
  limitReached: boolean;
};
/** Add the complete requested set or leave it unchanged: no silent truncation at the limit. */
export function changeCandidateSelection(
  selected: MediaCard[],
  requested: MediaCard[],
  checked: boolean,
): CandidateSelectionResult {
  const eligible = requested.filter(isSelectableCandidate);
  const ids = new Set(eligible.map((card) => card.media.id));
  if (!checked)
    return {
      cards: selected.filter((card) => !ids.has(card.media.id)),
      limitReached: false,
    };
  const next = new Map(
    selected.filter(isSelectableCandidate).map((card) => [card.media.id, card]),
  );
  for (const card of eligible)
    if (!next.has(card.media.id)) next.set(card.media.id, card);
  if (next.size > MAX_SCRAPE_QUEUE)
    return { cards: selected, limitReached: true };
  return { cards: [...next.values()], limitReached: false };
}
