import { expect, it } from "vitest";
import type { MediaCard, MediaKind } from "../../api/library";
import {
  changeCandidateSelection,
  isSelectableCandidate,
  ordinaryCandidates,
} from "./candidate-model";

function card(id: string, kind: MediaKind = "movie"): MediaCard {
  return {
    media: {
      id,
      kind,
      title: "Synthetic title",
      original_title: null,
      year: null,
      created_at: "2026-01-01",
      updated_at: "2026-01-01",
    },
    poster_uri: null,
    health: [],
  };
}
it("excludes a private ID even when a mixed response repeats it as an ordinary record", () => {
  expect(
    ordinaryCandidates([
      card("same"),
      card("same", "adult_video"),
      card("ordinary"),
      card("ordinary"),
    ]),
  ).toEqual([card("ordinary")]);
});
it("never admits private, unsupported, empty or control-character IDs to the scraper queue", () => {
  const invalid = [
    card("private", "adult_video"),
    card("episode", "tv_episode"),
    card(""),
    card("bad id"),
    card("bad\u0000id"),
    card("x".repeat(129)),
  ];
  for (const candidate of invalid)
    expect(isSelectableCandidate(candidate)).toBe(false);
  expect(
    changeCandidateSelection(
      [],
      [...invalid, card("ordinary"), card("ordinary")],
      true,
    ),
  ).toEqual({ cards: [card("ordinary")], limitReached: false });
});
