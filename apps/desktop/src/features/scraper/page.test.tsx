import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { libraryApi } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
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

vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/provider-auth", () => ({
  providerAuthApi: { status: vi.fn() },
}));
vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { snapshot: vi.fn() },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { toolsMedia: vi.fn(), toolsDetail: vi.fn() },
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
function show(mediaId?: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const content = (id?: string) => (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <ScraperPage mediaId={id} />
      </QueryClientProvider>
    </I18nextProvider>
  );
  const view = render(content(mediaId));
  return { ...view, changeTarget: (id?: string) => view.rerender(content(id)) };
}
const localFilm = {
  id: "media-1",
  title: "Local film",
  kind: "movie" as const,
  original_title: null,
  year: 2024,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
};
function localDetail(id = "media-1") {
  return {
    media: { ...localFilm, id },
    files: [],
    artworks: [],
    external_ids: [],
    metadata_values: [],
    health: [],
    relations: [],
    poster_uri: null,
  };
}
beforeEach(async () => {
  vi.resetAllMocks();
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    provider_id: "tmdb",
    supported_methods: ["bearer"],
    method: "bearer",
    storage: "system",
    configured: true,
    status: "configured",
  });
  vi.mocked(libraryApi.toolsDetail).mockImplementation(async (id) =>
    localDetail(id),
  );
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
  await screen.findByText("Local film · 2024");
  fireEvent.change(screen.getByLabelText("Local media item"), {
    target: { value: "media-1" },
  });
  await waitFor(() =>
    expect(screen.getByLabelText("Title")).toHaveValue("Local film"),
  );
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Synthetic film" },
  });
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
  await screen.findByText("TMDB");
  expect(scraperApi.search).not.toHaveBeenCalled();
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  expect(scraperApi.search).not.toHaveBeenCalled();
});
it("localizes the workflow in Chinese and snapshots the query language", async () => {
  await i18n.changeLanguage("zh-CN");
  show();
  await screen.findByText("TMDB");
  expect(screen.getByText("刮削工作台")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("影片名称"), {
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
  await screen.findByText("TMDB");
  fireEvent.change(screen.getByLabelText("Media type"), {
    target: { value: "tv" },
  });
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Synthetic series" },
  });
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
  await screen.findByText(/AniList/);
  fireEvent.change(screen.getByLabelText("Media type"), {
    target: { value: "anime" },
  });
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Synthetic anime" },
  });
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
  expect(scraperApi.searchPreview).toHaveBeenCalledWith("saved-run");
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

it("prefills and locks an explicit local target without issuing a provider search", async () => {
  show("media-1");
  await waitFor(() =>
    expect(screen.getByLabelText("Title")).toHaveValue("Local film"),
  );
  expect(screen.getByLabelText("Release year (optional)")).toHaveValue(2024);
  expect(screen.getByLabelText("Media type")).toBeDisabled();
  expect(screen.queryByLabelText("Local media item")).not.toBeInTheDocument();
  expect(libraryApi.toolsDetail).toHaveBeenCalledWith("media-1");
  expect(scraperApi.search).not.toHaveBeenCalled();
  expect(
    screen.getByText("External identifiers (advanced)").closest("details"),
  ).not.toHaveAttribute("open");
});
it("ignores a late candidate detail after switching the actual local target", async () => {
  let finish!: (value: typeof syntheticMetadata) => void;
  vi.mocked(scraperApi.detail).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const view = show("media-1");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Search providers" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  await waitFor(() => expect(scraperApi.detail).toHaveBeenCalled());
  view.changeTarget("media-2");
  await waitFor(() =>
    expect(libraryApi.toolsDetail).toHaveBeenCalledWith("media-2"),
  );
  await act(async () => {
    finish(syntheticMetadata);
  });
  expect(
    screen.queryByText("Selected metadata: Synthetic film"),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Preview merge" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.preview).not.toHaveBeenCalled();
});
it("discards a sidecar preview that arrives after target context is left", async () => {
  const view = await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Confirm selected metadata" }),
  );
  let finish!: (value: typeof syntheticOutput) => void;
  vi.mocked(scraperOutputApi.preview).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  fireEvent.click(
    await screen.findByRole("button", {
      name: "Preview NFO and poster output",
    }),
  );
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenCalled());
  view.changeTarget("media-2");
  await act(async () => {
    finish(syntheticOutput);
  });
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledWith("output-1"),
  );
  expect(
    screen.queryByText("/synthetic/library/film.nfo"),
  ).not.toBeInTheDocument();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
it("cancels merge and output previews without any writes", async () => {
  await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Cancel merge preview" }),
  );
  expect(
    screen.queryByRole("button", { name: "Confirm selected metadata" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Confirm selected metadata" }),
  );
  fireEvent.click(
    await screen.findByRole("button", {
      name: "Preview NFO and poster output",
    }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Cancel output preview" }),
  );
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledWith("output-1"),
  );
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
it("requires configured credentials and offers manual setup without enabling providers", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "tmdb";
  snapshot.providers[0].descriptor.requires_api_key = true;
  snapshot.providers[0].config.enabled = true;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    provider_id: "tmdb",
    supported_methods: ["bearer"],
    method: "bearer",
    storage: "system",
    configured: false,
    status: "not_configured",
  });
  show("media-1");
  await screen.findByText("TMDB (Credential required)");
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("link", { name: "Configure credentials" }),
  ).toHaveAttribute("href", "/settings");
  expect(scraperApi.search).not.toHaveBeenCalled();
});
it("rejects adult and unsupported local targets instead of defaulting to movie", async () => {
  vi.mocked(libraryApi.toolsDetail).mockResolvedValue({
    ...localDetail(),
    media: { ...localFilm, title: "PRIVATE TITLE", kind: "adult_video" },
  });
  show("media-1");
  await screen.findByRole("alert");
  expect(screen.queryByText("PRIVATE TITLE")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Title")).toHaveValue("");
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  expect(scraperApi.search).not.toHaveBeenCalled();
});

it("filters private and unsupported titles out of local target choices", async () => {
  vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
    items: [
      {
        media: {
          ...localFilm,
          id: "private",
          kind: "adult_video",
          title: "PRIVATE OPTION",
        },
        poster_uri: null,
        health: [],
      },
      {
        media: {
          ...localFilm,
          id: "music",
          kind: "music_release",
          title: "MUSIC OPTION",
        },
        poster_uri: null,
        health: [],
      },
    ],
    next_cursor: null,
  });
  show();
  await waitFor(() =>
    expect(screen.getByLabelText("Local media item")).toBeEnabled(),
  );
  expect(screen.queryByText("PRIVATE OPTION · 2024")).not.toBeInTheDocument();
  expect(screen.queryByText("MUSIC OPTION · 2024")).not.toBeInTheDocument();
});
it("keeps disabled sources disabled and allows an explicitly selected public source", async () => {
  const snapshot = fixtureSnapshot();
  const tmdb = snapshot.providers[0];
  tmdb.descriptor.id = "tmdb";
  tmdb.config.enabled = false;
  const wiki = {
    ...tmdb,
    descriptor: { ...tmdb.descriptor, id: "wikipedia.html" },
    config: { ...tmdb.config, enabled: true },
  };
  snapshot.providers.push(wiki);
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  show();
  await screen.findByText("TMDB (Disabled)");
  expect(
    screen.getByRole("checkbox", { name: "TMDB (Disabled)" }),
  ).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Synthetic article" },
  });
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  expect(
    screen.queryByText("Wikipedia requires the exact article title."),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "Wikipedia" }));
  expect(screen.getByLabelText("Title")).toHaveAccessibleDescription(
    "Wikipedia requires the exact article title.",
  );
  expect(
    screen.getByText("Wikipedia requires the exact article title."),
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await waitFor(() =>
    expect(scraperApi.search).toHaveBeenCalledWith(
      expect.objectContaining({ providers: ["wikipedia.html"] }),
    ),
  );
  expect(providerAuthApi.status).not.toHaveBeenCalled();
});

it("invalidates a merge preview when a local target selection changes", async () => {
  await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview merge" }));
  await screen.findByRole("button", { name: "Confirm selected metadata" });
  fireEvent.change(screen.getByLabelText("Local media item"), {
    target: { value: "" },
  });
  expect(
    screen.queryByRole("button", { name: "Confirm selected metadata" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText("Selected metadata: Synthetic film"),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
it("cancels a pending candidate selection and ignores its late response", async () => {
  let finish!: (value: typeof syntheticMetadata) => void;
  vi.mocked(scraperApi.detail).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  show("media-1");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Search providers" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Cancel candidate selection" }),
  );
  await act(async () => {
    finish(syntheticMetadata);
  });
  expect(
    screen.queryByText("Selected metadata: Synthetic film"),
  ).not.toBeInTheDocument();
  expect(scraperApi.preview).not.toHaveBeenCalled();
});
