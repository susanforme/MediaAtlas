import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { libraryApi } from "../../api/library";
import { providerApi } from "../../api/providers";
import { scraperApi, scraperOutputApi } from "../../api/scraper";
import i18n from "../../i18n";
import { fixtureSnapshot } from "../providers/fixtures";
import {
  syntheticMerge,
  syntheticMetadata,
  syntheticOutput,
  syntheticSearch,
} from "./fixtures";
import { ScraperPage } from "./page";

vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { snapshot: vi.fn() },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { toolsMedia: vi.fn() },
}));
vi.mock("../../api/scraper", async (original) => ({
  ...(await original<typeof import("../../api/scraper")>()),
  scraperApi: {
    search: vi.fn(),
    searchPreview: vi.fn(),
    detail: vi.fn(),
    preview: vi.fn(),
    confirm: vi.fn(),
    artworkPreview: vi.fn(),
  },
  scraperOutputApi: {
    history: vi.fn(),
    preview: vi.fn(),
    execute: vi.fn(),
    discard: vi.fn(),
  },
}));
function show() {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <ScraperPage />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "tmdb";
  snapshot.providers[0].descriptor.media_kinds = ["movie", "tv_series"];
  snapshot.providers[0].descriptor.fixture = false;
  snapshot.providers[0].config.enabled = true;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  vi.mocked(scraperApi.search).mockResolvedValue(syntheticSearch);
  vi.mocked(scraperApi.searchPreview).mockResolvedValue(syntheticSearch);
  vi.mocked(scraperApi.detail).mockResolvedValue(syntheticMetadata);
  vi.mocked(scraperApi.preview).mockResolvedValue(syntheticMerge);
  vi.mocked(scraperApi.confirm).mockResolvedValue({
    schema_version: 1,
    preview_id: "merge-1",
    applied_fields: 1,
    already_applied: false,
  });
  vi.mocked(scraperApi.artworkPreview).mockResolvedValue({
    thumbnail: "data:image/png;base64,aGVsbG8=",
  });
  vi.mocked(scraperOutputApi.history).mockResolvedValue([]);
  vi.mocked(scraperOutputApi.discard).mockResolvedValue(null);
  vi.mocked(scraperOutputApi.preview).mockResolvedValue(syntheticOutput);
  vi.mocked(scraperOutputApi.execute).mockResolvedValue({
    schema_version: 1,
    preview_id: "output-1",
    written: 1,
    preserved: 0,
    already_applied: false,
    state: "succeeded",
  });
  vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
    items: [
      {
        media: {
          id: "media-1",
          title: "Local film",
          kind: "movie",
          original_title: null,
          year: 2024,
          created_at: "2026-01-01T00:00:00Z",
          updated_at: "2026-01-01T00:00:00Z",
        },
        poster_uri: null,
        health: [],
      },
    ],
    next_cursor: null,
  });
});
async function searchAndChoose() {
  const view = show();
  await screen.findByText("tmdb");
  fireEvent.change(
    screen.getByLabelText("Title or exact Wikipedia article title"),
    { target: { value: "Synthetic film" } },
  );
  fireEvent.change(screen.getByLabelText("Release year (optional)"), {
    target: { value: "2023" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await screen.findByText("Release year conflicts");
  expect(scraperApi.detail).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByText("Selected metadata: Synthetic film");
  await screen.findByText("Local film · 2024");
  fireEvent.change(screen.getByLabelText("Local media item"), {
    target: { value: "media-1" },
  });
  return view;
}
it("keeps search, explicit candidate selection, protected merge and sidecar execution separate", async () => {
  await searchAndChoose();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  expect(scraperApi.artworkPreview).not.toHaveBeenCalled();
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  await screen.findByText("Protected: locked or manually edited");
  expect(screen.getByText("My manual title")).toBeInTheDocument();
  expect(scraperApi.preview).toHaveBeenCalledWith({
    run_id: "run-1",
    candidate_index: 0,
    media_item_id: "media-1",
    selected_fields: ["title", "overview"],
    selected_artwork: [],
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm selected metadata" }),
  );
  await screen.findByText("Metadata confirmed. 1 fields applied.");
  expect(scraperApi.confirm).toHaveBeenCalledWith("merge-1");
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Preview NFO and poster output" }),
  );
  await screen.findByText("/synthetic/library/film.nfo");
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  expect(
    screen.getByText("Existing local metadata is protected"),
  ).toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and write sidecars" }),
  );
  await screen.findByText("Output recorded: 1 written, 0 preserved.");
  expect(scraperOutputApi.execute).toHaveBeenCalledWith("output-1");
});
it("loads only an explicitly requested validated thumbnail without changing artwork selection", async () => {
  await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview image" }));
  await screen.findByRole("img", { name: "poster preview" });
  expect(scraperApi.artworkPreview).toHaveBeenCalledWith("run-1", 0, 0);
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  await waitFor(() =>
    expect(scraperApi.preview).toHaveBeenCalledWith(
      expect.objectContaining({ selected_artwork: [] }),
    ),
  );
});
it("does not search automatically and validates input before invoking providers", async () => {
  show();
  await screen.findByText("tmdb");
  expect(scraperApi.search).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await screen.findByRole("alert");
  expect(scraperApi.search).not.toHaveBeenCalled();
});
it("localizes the workflow in Chinese and snapshots the query language", async () => {
  await i18n.changeLanguage("zh-CN");
  show();
  await screen.findByText("tmdb");
  expect(screen.getByText("刮削工作台")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("标题或准确的维基百科条目名"), {
    target: { value: "动画标题" },
  });
  fireEvent.click(screen.getByRole("button", { name: "检索来源" }));
  await waitFor(() =>
    expect(scraperApi.search).toHaveBeenCalledWith(
      expect.objectContaining({
        query: expect.objectContaining({
          language: "zh-CN",
          title: "动画标题",
        }),
      }),
    ),
  );
  await screen.findByText("发行年份冲突");
  expect(
    screen.getByRole("button", { name: "选择此候选" }),
  ).toBeInTheDocument();
});
it("shows desktop-required failures without synthetic production results", async () => {
  vi.mocked(providerApi.snapshot).mockRejectedValue(
    new DesktopError("desktop_required"),
  );
  vi.mocked(scraperOutputApi.history).mockRejectedValue(
    new DesktopError("desktop_required"),
  );
  show();
  await screen.findAllByText("Open in the desktop app");
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  expect(screen.queryByText("Synthetic film")).not.toBeInTheDocument();
});
it("does not confirm a preview containing only protected fields", async () => {
  vi.mocked(scraperApi.preview).mockResolvedValue({
    ...syntheticMerge,
    fields: [syntheticMerge.fields[0]],
  });
  await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  await screen.findByText("Protected: locked or manually edited");
  expect(
    screen.getByRole("button", { name: "Confirm selected metadata" }),
  ).toBeDisabled();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
});
it("renders TV seasons with their explicit aired numbering", async () => {
  vi.mocked(scraperApi.search).mockResolvedValue({
    ...syntheticSearch,
    candidates: [
      {
        ...syntheticSearch.candidates[0],
        candidate: {
          kind: "tv",
          data: syntheticSearch.candidates[0].candidate.data,
        },
      },
    ],
  });
  vi.mocked(scraperApi.detail).mockResolvedValue({
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
          title: "Second season",
          episodes: [
            {
              remote_id: "episode-1",
              season_number: 2,
              episode_number: 1,
              title: "Synthetic episode",
              overview: null,
              order: "aired",
            },
          ],
        },
      ],
    },
  });
  show();
  await screen.findByText("tmdb");
  fireEvent.change(screen.getByLabelText("Media type"), {
    target: { value: "tv" },
  });
  fireEvent.change(
    screen.getByLabelText("Title or exact Wikipedia article title"),
    { target: { value: "Synthetic series" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await screen.findByRole("button", { name: "Choose this candidate" });
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByText("Episode order: Aired");
  expect(screen.getByText("Season 2 · Second season")).toBeInTheDocument();
  expect(screen.getByText("Episode 1: Synthetic episode")).toBeInTheDocument();
});
it("uses the independent Anime query and displays aliases", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "anilist";
  snapshot.providers[0].descriptor.media_kinds = ["anime"];
  snapshot.providers[0].config.enabled = true;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  vi.mocked(scraperApi.search).mockResolvedValue({
    ...syntheticSearch,
    candidates: [
      {
        ...syntheticSearch.candidates[0],
        candidate: {
          kind: "anime",
          data: { ...syntheticMetadata.data.identity, provider_id: "anilist" },
        },
      },
    ],
  });
  vi.mocked(scraperApi.detail).mockResolvedValue({
    kind: "anime",
    data: {
      identity: { ...syntheticMetadata.data.identity, provider_id: "anilist" },
      fields: [],
      artwork: [],
      format: "TV",
      season: "SPRING",
      episodes: 12,
      aliases: ["Synthetic alias"],
    },
  });
  show();
  await screen.findByText("anilist");
  fireEvent.change(screen.getByLabelText("Media type"), {
    target: { value: "anime" },
  });
  fireEvent.change(
    screen.getByLabelText("Title or exact Wikipedia article title"),
    { target: { value: "Synthetic anime" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await screen.findByRole("button", { name: "Choose this candidate" });
  expect(scraperApi.search).toHaveBeenCalledWith(
    expect.objectContaining({
      query: expect.objectContaining({ kind: "anime" }),
      providers: ["anilist"],
    }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByText("Synthetic alias");
  expect(screen.getByText("12")).toBeInTheDocument();
});
it("restores saved candidates without issuing a new provider search", async () => {
  vi.mocked(scraperOutputApi.history).mockResolvedValue([
    {
      id: "saved-run",
      query: {
        kind: "movie",
        title: "Saved query",
        year: 2024,
        external_ids: [],
        language: "en-US",
      },
      state: "succeeded",
      created_at: "2026-01-01T00:00:00Z",
      candidate_count: 1,
    },
  ]);
  show();
  await screen.findByRole("button", { name: "Restore candidates" });
  fireEvent.click(screen.getByRole("button", { name: "Restore candidates" }));
  await screen.findByText(
    "Saved candidates restored. No new provider search was sent.",
  );
  expect(scraperApi.searchPreview).toHaveBeenCalledWith(
    "saved-run",
    expect.anything(),
  );
  expect(scraperApi.search).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByText("Selected metadata: Synthetic film");
  expect(scraperApi.detail).toHaveBeenCalledWith("run-1", 0);
});
it("discards an abandoned output preview on component unmount without executing it", async () => {
  const view = await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  await screen.findByText("Protected: locked or manually edited");
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm selected metadata" }),
  );
  await screen.findByText("Metadata confirmed. 1 fields applied.");
  fireEvent.click(
    screen.getByRole("button", { name: "Preview NFO and poster output" }),
  );
  await screen.findByText("/synthetic/library/film.nfo");
  view.unmount();
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledWith("output-1"),
  );
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
