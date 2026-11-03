import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { musicApi } from "../../api/music";
import i18n from "../../i18n";
import {
  syntheticMusicGraph,
  syntheticTagPreview,
  syntheticTagResult,
} from "./fixtures";
import {
  MusicAlbumPage,
  MusicAlbumTools,
  MusicArtistPage,
  MusicPage,
  MusicToolsPage,
  TagEditor,
} from "./page";

vi.mock("../../api/music", async (original) => ({
  ...(await original<typeof import("../../api/music")>()),
  musicApi: {
    library: vi.fn(),
    toolsLibrary: vi.fn(),
    toolsAlbum: vi.fn(),
    openFile: vi.fn(),
    search: vi.fn(),
    release: vi.fn(),
    import: vi.fn(),
    album: vi.fn(),
    artist: vi.fn(),
    localFiles: vi.fn(),
    linkTrack: vi.fn(),
    coverPreview: vi.fn(),
    cachedCover: vi.fn(),
    read: vi.fn(),
    history: vi.fn(),
    preview: vi.fn(),
    execute: vi.fn(),
    restorePreview: vi.fn(),
    restoreExecute: vi.fn(),
    discard: vi.fn(),
  },
}));
function show(ui: React.ReactNode) {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        {ui}
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  vi.mocked(musicApi.toolsLibrary).mockResolvedValue([]);
  vi.mocked(musicApi.library).mockResolvedValue([]);
  vi.mocked(musicApi.toolsAlbum).mockResolvedValue(syntheticMusicGraph);
  vi.mocked(musicApi.cachedCover).mockResolvedValue(null);
  vi.mocked(musicApi.openFile).mockResolvedValue({ requested: true });
  vi.mocked(musicApi.album).mockResolvedValue(syntheticMusicGraph);
  vi.mocked(musicApi.artist).mockResolvedValue({
    artist: syntheticMusicGraph.artists[0],
    releases: [],
  });
  vi.mocked(musicApi.search).mockResolvedValue([
    {
      mbid: "edition-jp",
      title: "Same album",
      artist: "Artist",
      country: "JP",
      date: "2024",
      disambiguation: "Deluxe",
      medium_count: 2,
      track_count: 20,
    },
    {
      mbid: "edition-us",
      title: "Same album",
      artist: "Artist",
      country: "US",
      date: "2000",
      disambiguation: null,
      medium_count: 1,
      track_count: 10,
    },
  ]);
  vi.mocked(musicApi.release).mockResolvedValue(syntheticMusicGraph);
  vi.mocked(musicApi.import).mockResolvedValue(syntheticMusicGraph);
  vi.mocked(musicApi.localFiles).mockResolvedValue([
    {
      file_id: "file-1",
      media_item_id: "local-1",
      relative_path: "disc1/track.flac",
    },
  ]);
  vi.mocked(musicApi.linkTrack).mockResolvedValue(null);
  vi.mocked(musicApi.read).mockResolvedValue({
    file_id: "file-1",
    tags: { title: "Local title" },
    format: "FLAC",
    writable: true,
    picture_count: 1,
  });
  vi.mocked(musicApi.history).mockResolvedValue([]);
  vi.mocked(musicApi.preview).mockResolvedValue(syntheticTagPreview);
  vi.mocked(musicApi.execute).mockResolvedValue(syntheticTagResult);
  vi.mocked(musicApi.discard).mockResolvedValue(null);
  vi.mocked(musicApi.restorePreview).mockResolvedValue({
    ...syntheticTagPreview,
    id: "restore-1",
    kind: "restore",
  });
  vi.mocked(musicApi.restoreExecute).mockResolvedValue({
    ...syntheticTagResult,
    operation_id: "restore-op",
  });
});
it("requires an explicit release choice and import while displaying edition differences", async () => {
  show(<MusicToolsPage />);
  fireEvent.change(screen.getByRole("textbox", { name: "Artist or album" }), {
    target: { value: "Same album" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Search MusicBrainz" }));
  await screen.findByText(/Deluxe/);
  expect(screen.getByText(/US · 2000/)).toBeVisible();
  expect(musicApi.release).not.toHaveBeenCalled();
  expect(musicApi.import).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getAllByRole("button", { name: "Inspect this edition" })[0],
  );
  await screen.findByRole("button", { name: "Import selected release" });
  expect(musicApi.release).toHaveBeenCalledWith(
    "edition-jp",
    expect.anything(),
  );
  expect(musicApi.import).not.toHaveBeenCalled();
  expect(screen.getByText("Disc 2 Bonus CD")).toBeVisible();
  fireEvent.click(
    screen.getByRole("button", { name: "Import selected release" }),
  );
  await waitFor(() =>
    expect(musicApi.import).toHaveBeenCalledWith(
      "edition-jp",
      expect.anything(),
    ),
  );
});
it("shows online/local Diff and gates writes and restores behind separate confirmation", async () => {
  show(<TagEditor fileId="file-1" graph={syntheticMusicGraph} />);
  await screen.findByText("Local title");
  expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue(
    "Online track",
  );
  expect(
    screen.getByRole("button", { name: "Preview tag changes" }),
  ).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "Write Title" }));
  fireEvent.click(screen.getByRole("button", { name: "Preview tag changes" }));
  const preview = await screen.findByRole("region", {
    name: "Preview tag changes",
  });
  expect(musicApi.preview).toHaveBeenCalledWith("file-1", {
    title: "Online track",
  });
  expect(musicApi.execute).not.toHaveBeenCalled();
  expect(
    within(preview).getByRole("button", { name: "Confirm and write tags" }),
  ).toBeDisabled();
  fireEvent.click(within(preview).getByRole("checkbox"));
  fireEvent.click(
    within(preview).getByRole("button", { name: "Confirm and write tags" }),
  );
  await screen.findByText("Tag operation recorded: Completed");
  expect(musicApi.execute).toHaveBeenCalledWith("preview-1");
  fireEvent.click(
    screen.getByRole("button", { name: "Preview restore from backup" }),
  );
  const restore = await screen.findByRole("button", {
    name: "Confirm restore",
  });
  expect(restore).toBeDisabled();
  expect(musicApi.restoreExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("checkbox", { name: /I confirm restoring/ }),
  );
  fireEvent.click(restore);
  await waitFor(() =>
    expect(musicApi.restoreExecute).toHaveBeenCalledWith("restore-1"),
  );
});
it("discards leased previews on unmount and restores persisted backups after reopening", async () => {
  vi.mocked(musicApi.history).mockResolvedValue([syntheticTagResult]);
  const view = show(<TagEditor fileId="file-1" graph={syntheticMusicGraph} />);
  await screen.findByText(/Saved operation: op-1/);
  fireEvent.click(
    screen.getByRole("button", { name: "Preview restore from backup" }),
  );
  await screen.findByRole("button", { name: "Confirm restore" });
  view.unmount();
  await waitFor(() =>
    expect(musicApi.discard).toHaveBeenCalledWith("restore-1"),
  );
  expect(musicApi.restoreExecute).not.toHaveBeenCalled();
});
it("renders readonly album discs, sources and persisted cover without provider actions", async () => {
  vi.mocked(musicApi.cachedCover).mockResolvedValue({
    thumbnail: "data:image/png;base64,aGVsbG8=",
    source_url: "https://coverartarchive.org/release/mbid-1",
    license: "Unknown; original rights holder",
  });
  show(<MusicAlbumPage releaseId="release-1" />);
  await screen.findAllByText("Bonus track");
  expect(await screen.findByText(/保护标题/)).toBeVisible();
  expect(
    screen.queryByRole("button", { name: "Load cover through backend" }),
  ).not.toBeInTheDocument();
  expect(await screen.findByRole("img")).toHaveAttribute(
    "src",
    "data:image/png;base64,aGVsbG8=",
  );
  expect(musicApi.cachedCover).toHaveBeenCalledWith("release-1");
  expect(musicApi.coverPreview).not.toHaveBeenCalled();
});
it("shows artist aliases and Chinese controls with read-only format protection", async () => {
  await i18n.changeLanguage("zh-CN");
  show(<MusicArtistPage artistId="artist-1" />);
  expect(await screen.findByText(/合成艺人/)).toBeVisible();
  vi.mocked(musicApi.read).mockResolvedValue({
    file_id: "file-1",
    tags: {},
    format: "M4A",
    writable: false,
    picture_count: 0,
  });
  show(<TagEditor fileId="file-1" graph={syntheticMusicGraph} />);
  await screen.findByText(/只读/);
  fireEvent.click(screen.getByRole("checkbox", { name: "写入标题" }));
  expect(screen.getByRole("button", { name: "预览标签修改" })).toBeDisabled();
});
it("links a real selected scanned file without writing its tags", async () => {
  show(<MusicAlbumTools releaseId="release-1" />);
  await screen.findByRole("combobox", { name: "Select a track" });
  fireEvent.change(screen.getByRole("combobox", { name: "Select a track" }), {
    target: { value: "track-1" },
  });
  fireEvent.change(
    await screen.findByRole("combobox", { name: "Select an audio file" }),
    { target: { value: "file-1" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Link file to track" }));
  await waitFor(() =>
    expect(musicApi.linkTrack).toHaveBeenCalledWith("track-1", "file-1"),
  );
  expect(musicApi.preview).not.toHaveBeenCalled();
  expect(musicApi.execute).not.toHaveBeenCalled();
});
it("keeps an immutable preview frozen and allows cancellation without execution", async () => {
  show(<TagEditor fileId="file-1" graph={syntheticMusicGraph} />);
  await screen.findByText("Local title");
  fireEvent.click(screen.getByRole("checkbox", { name: "Write Title" }));
  fireEvent.click(screen.getByRole("button", { name: "Preview tag changes" }));
  await screen.findByRole("button", { name: "Confirm and write tags" });
  expect(screen.getByRole("textbox", { name: "Title" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Discard preview" }));
  await waitFor(() =>
    expect(musicApi.discard).toHaveBeenCalledWith("preview-1"),
  );
  expect(musicApi.execute).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "Title" })).toBeEnabled();
});

it("keeps Music library and album read only and delegates local opening to the system", async () => {
  const library = show(<MusicPage />);
  await screen.findByText("No music releases yet");
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Search MusicBrainz" }),
  ).not.toBeInTheDocument();
  library.unmount();
  show(<MusicAlbumPage releaseId="release-1" />);
  await screen.findAllByText("Bonus track");
  expect(
    screen.queryByRole("button", { name: "Link file to track" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Preview tag changes" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Confirm and write tags" }),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await screen.findByText("Requested opening in system app");
  expect(musicApi.openFile).toHaveBeenCalledWith("file-1", expect.anything());
  expect(musicApi.read).not.toHaveBeenCalled();
  expect(musicApi.execute).not.toHaveBeenCalled();
});

it("loads and saves provider covers only in the separate Music tools", async () => {
  vi.mocked(musicApi.coverPreview).mockResolvedValue({
    thumbnail: "data:image/png;base64,aGVsbG8=",
    source_url: "https://coverartarchive.org/release/mbid-1",
    license: "Original image rights apply",
  });
  show(<MusicAlbumTools releaseId="release-1" />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Load cover through backend" }),
  );
  expect(await screen.findByRole("img")).toHaveAttribute(
    "src",
    "data:image/png;base64,aGVsbG8=",
  );
  expect(musicApi.coverPreview).toHaveBeenCalledWith("mbid-1", 0, "release-1");
  expect(musicApi.cachedCover).not.toHaveBeenCalled();
});
