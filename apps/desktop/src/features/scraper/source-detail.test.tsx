import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type MediaDetail } from "../../api/library";
import { scraperApi, type VideoMetadata } from "../../api/scraper";
import i18n from "../../i18n";
import { MediaDetailPage } from "../library/detail";
import { syntheticMetadata } from "./fixtures";
import { LibraryVideoSourceDetail, VideoSourceDetails } from "./source-detail";

vi.mock("../../api/scraper", async (original) => ({
  ...(await original<typeof import("../../api/scraper")>()),
  scraperApi: { mediaDetail: vi.fn() },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { detail: vi.fn() },
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <a href="/library">{children}</a>
  ),
}));
function show(component: React.ReactNode) {
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        {component}
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
const localDetail: MediaDetail = {
  media: {
    id: "media-1",
    title: "My protected library title",
    original_title: null,
    year: 2024,
    kind: "movie",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  },
  files: [],
  artworks: [],
  external_ids: [],
  metadata_values: [
    {
      id: "metadata-1",
      field_key: "title",
      locale: "en-US",
      value: "My protected library title",
      provider_id: null,
      source_url: null,
      is_user_modified: true,
      is_locked: true,
    },
  ],
  relations: [],
  health: [],
  poster_uri: null,
};
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  vi.mocked(scraperApi.mediaDetail).mockResolvedValue(syntheticMetadata);
  vi.mocked(libraryApi.detail).mockResolvedValue(localDetail);
});
it("adds confirmed Movie source details without replacing the protected library title", async () => {
  show(<MediaDetailPage mediaId="media-1" />);
  await screen.findByText("Confirmed video source details");
  await screen.findByText("Runtime (minutes)");
  expect(
    screen.getByRole("heading", { name: "My protected library title" }),
  ).toBeInTheDocument();
  expect(screen.getByText("90")).toBeInTheDocument();
  expect(
    screen.getByText(syntheticMetadata.data.identity.source_url),
  ).toBeInTheDocument();
  expect(screen.getByText("Locked")).toBeInTheDocument();
  expect(scraperApi.mediaDetail).toHaveBeenCalledWith("media-1");
});
it("shows an explicit empty state for un-enriched local video", async () => {
  vi.mocked(scraperApi.mediaDetail).mockResolvedValue(null);
  show(<LibraryVideoSourceDetail mediaId="media-1" />);
  await screen.findByText("No confirmed source details");
  expect(screen.queryByText("Runtime (minutes)")).not.toBeInTheDocument();
});
it("renders confirmed TV seasons and their numbering scheme", () => {
  const metadata: VideoMetadata = {
    kind: "tv",
    data: {
      identity: syntheticMetadata.data.identity,
      fields: [],
      artwork: [],
      order: "aired",
      seasons: [
        {
          remote_id: "season-1",
          season_number: 2,
          title: "Synthetic second season",
          episodes: [
            {
              remote_id: "episode-1",
              season_number: 2,
              episode_number: 1,
              title: "Synthetic first episode",
              overview: "Episode summary",
              order: "aired",
            },
          ],
        },
      ],
    },
  };
  show(<VideoSourceDetails metadata={metadata} />);
  expect(screen.getByText("Episode order: Aired")).toBeInTheDocument();
  expect(
    screen.getByText("Season 2 · Synthetic second season"),
  ).toBeInTheDocument();
  expect(
    screen.getByText("Episode 1: Synthetic first episode"),
  ).toBeInTheDocument();
});
it("localizes Anime format, season, aliases and episode details in Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  show(
    <VideoSourceDetails
      metadata={{
        kind: "anime",
        data: {
          identity: syntheticMetadata.data.identity,
          fields: [],
          artwork: [],
          format: "TV",
          season: "SPRING",
          episodes: 12,
          aliases: ["独立动画别名"],
        },
      }}
    />,
  );
  expect(screen.getByText("电视动画")).toBeInTheDocument();
  expect(screen.getByText("春季")).toBeInTheDocument();
  expect(screen.getByText("独立动画别名")).toBeInTheDocument();
  expect(screen.getByText("12")).toBeInTheDocument();
});
it("does not request ordinary source details for adult library entries", async () => {
  vi.mocked(libraryApi.detail).mockResolvedValue({
    ...localDetail,
    media: { ...localDetail.media, kind: "adult_video" },
  });
  show(<MediaDetailPage mediaId="media-1" />);
  await screen.findByText("Adult media stays hidden");
  expect(scraperApi.mediaDetail).not.toHaveBeenCalled();
});
