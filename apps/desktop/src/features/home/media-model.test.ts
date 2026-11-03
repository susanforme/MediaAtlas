import { expect, it } from "vitest";
import type { MediaDetail } from "../../api/library";
import { featuredBackdrop, featuredDescription } from "./media-model";

const detail: MediaDetail = {
  media: {
    id: "fixture",
    kind: "movie",
    title: "Synthetic title",
    original_title: null,
    year: null,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  },
  poster_uri: null,
  files: [],
  health: [],
  external_ids: [],
  relations: [],
  metadata_values: [],
  artworks: [
    {
      id: "background",
      kind: "backdrop",
      cache_path: "/private/cache/image.jpg",
      source_url: "https://provider.invalid/image.jpg",
    },
  ],
};
it("honors desktop image CSP and never requests a provider URL or local filesystem path", () => {
  expect(featuredBackdrop(detail)).toBeUndefined();
  expect(
    featuredBackdrop({
      ...detail,
      artworks: [
        {
          ...detail.artworks[0],
          source_url: "file:///private/cache/image.jpg",
        },
      ],
    }),
  ).toBeUndefined();
  expect(
    featuredBackdrop({
      ...detail,
      artworks: [
        { ...detail.artworks[0], source_url: "data:image/png;base64,YQ==" },
      ],
    }),
  ).toBe("data:image/png;base64,YQ==");
});
it("omits absent and private metadata rather than fabricating synopsis or artwork", () => {
  expect(featuredDescription(detail)).toBeUndefined();
  const privateDetail = {
    ...detail,
    media: { ...detail.media, kind: "adult_video" as const },
    artworks: [
      { ...detail.artworks[0], source_url: "data:image/png;base64,YQ==" },
    ],
  };
  expect(featuredBackdrop(privateDetail)).toBeUndefined();
  expect(featuredDescription(privateDetail)).toBeUndefined();
});
