import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import type { MediaDetail } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import { scraperApi, type VideoMetadata } from "../../api/scraper";
import i18n from "../../i18n";
import { syntheticIdentity } from "../scraper/fixtures";
import { VideoDetail } from "./video-detail";

vi.mock("../../api/scraper", () => ({
  scraperApi: { mediaDetail: vi.fn() },
}));
vi.mock("../../api/organization", () => ({
  nativeOpenApi: { open: vi.fn() },
}));
vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn() },
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));

const detail: MediaDetail = {
  media: {
    id: "movie",
    kind: "movie",
    title: "Local film",
    original_title: "Original title",
    year: 2026,
    created_at: "2026-11-04T10:00:00Z",
    updated_at: "2026-11-04T10:00:00Z",
  },
  files: [
    {
      id: "actual-file",
      media_item_id: "movie",
      root_id: "actual-root",
      relative_path: "Local film/movie.mkv",
      size_bytes: 1024,
      modified_at: null,
      part: null,
    },
  ],
  artworks: [],
  external_ids: [],
  metadata_values: [
    {
      id: "overview",
      field_key: "overview",
      value: "Edited local overview",
      locale: "en-US",
      provider_id: "local_nfo",
      source_url: null,
      is_locked: true,
      is_user_modified: true,
    },
  ],
  relations: [],
  health: [],
  poster_uri: null,
};

function show(value: MediaDetail, source: VideoMetadata | null = null) {
  vi.mocked(scraperApi.mediaDetail).mockResolvedValue(source);
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <VideoDetail detail={value} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

beforeEach(async () => {
  vi.clearAllMocks();
  vi.mocked(nativeOpenApi.open).mockResolvedValue({ requested: true });
  await i18n.changeLanguage("en-US");
});

it("shows real movie details and opens only the explicitly chosen local file", async () => {
  const { container } = show(detail);
  expect(container.querySelector('[data-slot="media-detail"]')).not.toBeNull();
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
    "Local film",
  );
  expect(screen.getByText("Original title")).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "Local files" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(screen.queryByRole("tab", { name: "Seasons and episodes" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
  expect(container.querySelector("video,audio,iframe")).toBeNull();
  expect(container.querySelector("img")).toBeNull();
  fireEvent.click(
    screen.getByRole("button", {
      name: "Open in system app: Local film/movie.mkv",
    }),
  );
  await waitFor(() =>
    expect(nativeOpenApi.open).toHaveBeenCalledWith("actual-file", false),
  );
  fireEvent.click(screen.getByRole("tab", { name: "Metadata and sources" }));
  const panel = screen.getByRole("tabpanel");
  expect(within(panel).getByText("Locked")).toBeInTheDocument();
  expect(within(panel).getByText("User edited")).toBeInTheDocument();
  expect(within(panel).getByText("Edited local overview")).toBeInTheDocument();
});

it("browses source seasons without local file claims and preserves the source", async () => {
  const series: MediaDetail = {
    ...detail,
    media: { ...detail.media, id: "series", kind: "tv_series" },
  };
  show(series, {
    kind: "tv",
    data: {
      identity: syntheticIdentity,
      fields: [],
      artwork: [],
      order: "aired",
      seasons: [1, 2].map((season) => ({
        remote_id: `season-${season}`,
        season_number: season,
        title: `Season title ${season}`,
        episodes: [
          {
            remote_id: `episode-${season}`,
            season_number: season,
            episode_number: 1,
            title: `Episode title ${season}`,
            overview: null,
            order: "aired",
          },
        ],
      })),
    },
  });
  await screen.findByRole("button", { name: /Season 2/ });
  fireEvent.click(screen.getByRole("button", { name: /Season 2/ }));
  const panel = screen.getByRole("tabpanel");
  expect(within(panel).getByText("Episode title 2")).toBeInTheDocument();
  expect(within(panel).queryByText("Episode title 1")).toBeNull();
  expect(
    within(panel).queryByRole("button", { name: /Open in system app/ }),
  ).toBeNull();
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("tab", { name: "Metadata and sources" }));
  const metadata = within(screen.getByRole("tabpanel"));
  expect(metadata.getByText(syntheticIdentity.source_url)).toBeInTheDocument();
  expect(metadata.getByRole("heading", { name: /Aired/ })).toBeInTheDocument();
});

it("lets users expand long overviews and keeps absent season data explicit", async () => {
  show({
    ...detail,
    media: { ...detail.media, kind: "tv_series" },
    metadata_values: [
      { ...detail.metadata_values[0], value: "Long overview. ".repeat(30) },
    ],
  });
  const expand = screen.getByRole("button", { name: "Show more" });
  fireEvent.click(expand);
  expect(screen.getByRole("button", { name: "Show less" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  await waitFor(() =>
    expect(
      within(screen.getByRole("tabpanel")).getByText(
        "Season and episode details have not been fetched",
      ),
    ).toBeInTheDocument(),
  );
});
