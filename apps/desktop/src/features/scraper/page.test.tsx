import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { libraryApi } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import { type ReviewQueue, reviewQueueApi } from "../../api/review-queue";
import { scraperApi, scraperOutputApi } from "../../api/scraper";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { chooseSelectOption, selectOptions } from "../../test/select";
import { fixtureSnapshot } from "../providers/fixtures";
import {
  syntheticMerge,
  syntheticMetadata,
  syntheticOutput,
  syntheticSearch,
} from "./fixtures";
import { ScraperPage } from "./page";

vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));

const recoveryNavigate = vi.hoisted(() => vi.fn());
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => recoveryNavigate,
  Link: ({
    to,
    search,
    children,
  }: {
    to: string;
    search?: Record<string, string | string[] | undefined>;
    children: React.ReactNode;
  }) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).flatMap(([key, value]) =>
        value === undefined
          ? []
          : [[key, typeof value === "string" ? value : JSON.stringify(value)]],
      ),
    ).toString();
    return <a href={to + (query ? `?${query}` : "")}>{children}</a>;
  },
}));
vi.mock("../../api/provider-auth", () => ({
  providerAuthApi: { status: vi.fn() },
}));
vi.mock("../../api/review-queue", () => ({
  reviewQueueApi: { get: vi.fn(), update: vi.fn(), checkpoint: vi.fn() },
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
    defaultMode: vi.fn(),
    history: vi.fn(),
    preview: vi.fn(),
    execute: vi.fn(),
    discard: vi.fn(),
  },
}));
function show(
  mediaId?: string,
  queue?: string[],
  onQueueNavigate?: (mediaId: string) => void,
  sessionId?: string,
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const content = (id?: string) => (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <ScraperPage
          mediaId={id}
          queue={queue}
          sessionId={sessionId}
          onQueueNavigate={onQueueNavigate}
        />
      </QueryClientProvider>
    </I18nextProvider>
  );
  const view = render(content(mediaId));
  return { ...view, client, changeTarget: (id?: string) => view.rerender(content(id)) };
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
  vi.mocked(scraperApi.preview).mockImplementation(async (request) => ({
    ...syntheticMerge,
    media_item_id: request.media_item_id,
    fields: syntheticMerge.fields.map((field) => ({
      ...field,
      selected: request.selected_fields.includes(field.key) && !field.protected,
    })),
    artwork: request.selected_artwork.map(
      (index) => syntheticMetadata.data.artwork[index],
    ),
  }));
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
  vi.mocked(scraperOutputApi.defaultMode).mockResolvedValue("online");
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
async function waitForMergeReady() {
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: /^Confirm merge/ }),
    ).toBeEnabled(),
  );
}
async function confirmMerge() {
  await waitForMergeReady();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: /^Confirm merge/ }));
  });
}
async function openOutputPreview() {
  // Query notifications are asynchronous; observe confirmation before its next action.
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Metadata confirmed. 1 fields applied.",
    ),
  );
  const button = screen.getByRole("button", {
    name: "Preview NFO and poster output",
  });
  expect(button).toBeEnabled();
  await act(async () => {
    fireEvent.click(button);
  });
}
/** React can commit the enabled button before mutation options update in a passive effect. */
function clickFirstEnabledCommit(name: string | RegExp) {
  let clicks = 0;
  const observer = new MutationObserver(() => {
    const button = Array.from(
      document.querySelectorAll<HTMLButtonElement>("button"),
    ).find((element) => {
      const text = element.textContent?.trim() ?? "";
      return typeof name === "string" ? text === name : name.test(text);
    });
    if (!button || button.disabled) return;
    observer.disconnect();
    clicks += 1;
    // A native click preserves the real commit-to-passive-effect window.
    button.click();
  });
  observer.observe(document.body, {
    subtree: true,
    attributes: true,
    childList: true,
  });
  return { observer, clicks: () => clicks };
}

async function searchAndChoose() {
  const view = show();
  const localItem = screen.getByRole("combobox", { name: "Local media item" });
  await waitFor(() => expect(localItem).toBeEnabled());
  selectOptions(localItem);
  await screen.findByRole("option", { name: "Local film · 2024" });
  chooseSelectOption(localItem, "media-1");
  await waitFor(() =>
    expect(screen.getByLabelText("Title, link or ID")).toHaveValue("Local film"),
  );
  fireEvent.change(screen.getByLabelText("Title, link or ID"), {
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
  await screen.findByRole("region", { name: "Merge media metadata" });
  return view;
}
it("keeps search, explicit candidate selection, protected merge and sidecar execution separate", async () => {
  await searchAndChoose();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  expect(scraperApi.artworkPreview).not.toHaveBeenCalled();
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  await waitFor(() =>
    expect(screen.getByRole("checkbox", { name: /^Title/ })).toBeDisabled(),
  );
  expect(
    within(screen.getByRole("table")).getAllByText("My manual title").length,
  ).toBeGreaterThan(0);
  expect(scraperApi.preview).toHaveBeenCalledWith({
    run_id: "run-1",
    candidate_index: 0,
    media_item_id: "media-1",
    selected_fields: ["title", "overview"],
    selected_artwork: [],
  });
  await confirmMerge();
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Metadata confirmed. 1 fields applied.",
    ),
  );
  expect(scraperApi.confirm).toHaveBeenCalledWith("merge-1");
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  await openOutputPreview();
  await screen.findByText("/synthetic/library/film.nfo");
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  expect(
    screen.getByText("Existing local metadata is protected"),
  ).toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and write sidecars" }),
  );
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Output recorded: 1 written, 0 preserved.",
    ),
  );
  expect(scraperOutputApi.execute).toHaveBeenCalledWith("output-1");
});
it("loads only an explicitly requested validated thumbnail without changing artwork selection", async () => {
  await searchAndChoose();
  fireEvent.click(screen.getByRole("button", { name: "Preview image" }));
  await screen.findByRole("img", { name: "poster preview" });
  expect(scraperApi.artworkPreview).toHaveBeenCalledWith("run-1", 0, 0);
  await waitFor(() =>
    expect(scraperApi.preview).toHaveBeenCalledWith(
      expect.objectContaining({ selected_artwork: [] }),
    ),
  );
});
it("discards the previous NFO preview when its export mode changes and writes only the new proof", async () => {
  await searchAndChoose();
  await confirmMerge();
  await openOutputPreview();
  await screen.findByText("/synthetic/library/film.nfo");
  const mode = screen.getByRole("combobox", { name: "NFO export mode" });
  expect(mode).toHaveValue("online");
  chooseSelectOption(mode, "offline");
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledExactlyOnceWith("output-1"),
  );
  expect(screen.queryByText("/synthetic/library/film.nfo")).not.toBeInTheDocument();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  vi.mocked(scraperOutputApi.preview).mockResolvedValue({
    ...syntheticOutput,
    preview_id: "offline-output",
  });
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await screen.findByText("/synthetic/library/film.nfo");
  expect(scraperOutputApi.preview).toHaveBeenLastCalledWith("media-1", "offline");
  fireEvent.click(screen.getByRole("button", { name: "Confirm and write sidecars" }));
  await waitFor(() =>
    expect(scraperOutputApi.execute).toHaveBeenCalledExactlyOnceWith("offline-output"),
  );
});
it("inherits an offline directory default and keeps a one-off override local to the export", async () => {
  vi.mocked(scraperOutputApi.defaultMode).mockResolvedValue("offline");
  const view = await searchAndChoose();
  await confirmMerge();
  const mode = await screen.findByRole("combobox", { name: "NFO export mode" });
  await waitFor(() => expect(mode).toHaveValue("offline"));
  await openOutputPreview();
  await screen.findByText("/synthetic/library/film.nfo");
  expect(scraperOutputApi.preview).toHaveBeenLastCalledWith("media-1", undefined, "offline");
  chooseSelectOption(mode, "online");
  await waitFor(() => expect(scraperOutputApi.discard).toHaveBeenCalledExactlyOnceWith("output-1"));
  await act(async () => view.client.invalidateQueries({ queryKey: ["roots"] }));
  expect(mode).toHaveValue("online");
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenLastCalledWith("media-1", "online"));
});

it("blocks export while directory defaults fail and uses the retried default", async () => {
  vi.mocked(scraperOutputApi.defaultMode).mockRejectedValueOnce(new DesktopError("request_failed")).mockResolvedValue("offline");
  await searchAndChoose();
  await confirmMerge();
  const button = await screen.findByRole("button", { name: "Preview NFO and poster output" });
  await waitFor(() => expect(button).toBeDisabled());
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.getByRole("combobox", { name: "NFO export mode" })).toHaveValue("offline"));
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenCalledExactlyOnceWith("media-1", undefined, "offline"));
});

it("does not let a late directory default replace an explicitly chosen export mode", async () => {
  let finish!: (mode: "online" | "offline") => void;
  vi.mocked(scraperOutputApi.defaultMode).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  await searchAndChoose();
  await confirmMerge();
  const mode = await screen.findByRole("combobox", { name: "NFO export mode" });
  expect(screen.getByRole("button", { name: "Preview NFO and poster output" })).toBeDisabled();
  chooseSelectOption(mode, "offline");
  await act(async () => finish("online"));
  expect(mode).toHaveValue("offline");
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenCalledExactlyOnceWith("media-1", "offline"));
});

it("releases a preview when the inherited directory mode changes", async () => {
  const view = await searchAndChoose();
  await confirmMerge();
  await openOutputPreview();
  await screen.findByText("/synthetic/library/film.nfo");
  vi.mocked(scraperOutputApi.defaultMode).mockResolvedValue("offline");
  await act(async () => view.client.invalidateQueries({ queryKey: ["roots"] }));
  await waitFor(() => expect(screen.getByRole("combobox", { name: "NFO export mode" })).toHaveValue("offline"));
  await waitFor(() => expect(scraperOutputApi.discard).toHaveBeenCalledExactlyOnceWith("output-1"));
  expect(screen.queryByText("/synthetic/library/film.nfo")).not.toBeInTheDocument();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("refreshes a changed directory default instead of showing a preview for the old mode", async () => {
  await searchAndChoose();
  await confirmMerge();
  vi.mocked(scraperOutputApi.defaultMode).mockResolvedValue("offline");
  vi.mocked(scraperOutputApi.preview).mockRejectedValueOnce(new DesktopError("request_failed", {
    schema_version: 1, code: "conflict", message: "Conflict", correlation_id: "fixture",
  }));
  await openOutputPreview();
  await waitFor(() => expect(screen.getByRole("combobox", { name: "NFO export mode" })).toHaveValue("offline"));
  expect(screen.queryByText("/synthetic/library/film.nfo")).not.toBeInTheDocument();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await screen.findByText("/synthetic/library/film.nfo");
  expect(scraperOutputApi.preview).toHaveBeenLastCalledWith("media-1", undefined, "offline");
});

it("does not carry an override or a late directory default into another media target", async () => {
  let finish!: (mode: "online" | "offline") => void;
  vi.mocked(scraperOutputApi.defaultMode).mockImplementation((id) => id === "media-1"
    ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve("online"));
  vi.mocked(scraperOutputApi.preview).mockImplementation(async (id) => ({ ...syntheticOutput, media_item_id: id }));
  const view = show("media-1");
  await waitFor(() => expect(screen.getByRole("button", { name: "Search providers" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  fireEvent.click(await screen.findByRole("button", { name: "Choose this candidate" }));
  await confirmMerge();
  const firstMode = await screen.findByRole("combobox", { name: "NFO export mode" });
  chooseSelectOption(firstMode, "offline");
  expect(firstMode).toHaveValue("offline");
  view.changeTarget("media-2");
  await waitFor(() => expect(scraperOutputApi.defaultMode).toHaveBeenCalledWith("media-2"));
  await waitFor(() => expect(screen.getByRole("button", { name: "Search providers" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  fireEvent.click(await screen.findByRole("button", { name: "Choose this candidate" }));
  await confirmMerge();
  const secondMode = await screen.findByRole("combobox", { name: "NFO export mode" });
  await waitFor(() => expect(secondMode).toHaveValue("online"));
  await act(async () => finish("offline"));
  expect(secondMode).toHaveValue("online");
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenLastCalledWith("media-2", undefined, "online"));
});
it("shows the existing-NFO update consequence before the same explicit write confirmation", async () => {
  vi.mocked(scraperOutputApi.preview).mockResolvedValue({
    ...syntheticOutput,
    strategy: "update_nfo",
  });
  await searchAndChoose();
  await confirmMerge();
  await openOutputPreview();
  expect(await screen.findByText(
    "Confirmation updates the existing NFO. Video files are not modified.",
  )).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Confirm and write sidecars" })).toBeEnabled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("releases an old mode's late preview without replacing the current NFO proof", async () => {
  let finish!: (value: typeof syntheticOutput) => void;
  vi.mocked(scraperOutputApi.preview)
    .mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }))
    .mockResolvedValue({
      ...syntheticOutput,
      preview_id: "offline-output",
      files: syntheticOutput.files.map((file) => ({ ...file, destination: "/synthetic/offline.nfo" })),
    });
  await searchAndChoose();
  await confirmMerge();
  await openOutputPreview();
  await waitFor(() => expect(scraperOutputApi.preview).toHaveBeenCalledWith("media-1", undefined, "online"));
  const mode = screen.getByRole("combobox", { name: "NFO export mode" });
  expect(mode).toBeEnabled();
  chooseSelectOption(mode, "offline");
  fireEvent.click(screen.getByRole("button", { name: "Preview NFO and poster output" }));
  await screen.findByText("/synthetic/offline.nfo");
  await act(async () => finish(syntheticOutput));
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledExactlyOnceWith("output-1"),
  );
  expect(screen.queryByText("/synthetic/library/film.nfo")).not.toBeInTheDocument();
  expect(screen.getByText("/synthetic/offline.nfo")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Confirm and write sidecars" }));
  await waitFor(() =>
    expect(scraperOutputApi.execute).toHaveBeenCalledExactlyOnceWith("offline-output"),
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
it.each([
  ["title", "https://www.imdb.com/title/tt35538033/?ref_=fn_all_ttl_1"],
  ["title", "tt35538033"],
  ["external", "https://www.imdb.com/title/tt35538033/"],
  ["external", "tt35538033"],
])("submits an IMDb %s lookup without requiring a namespace: %s", async (input, identifier) => {
  show();
  await screen.findByText("TMDB");
  if (input === "external") {
    fireEvent.click(screen.getByText("External identifiers (advanced)"));
    expect(screen.getByLabelText("ID namespace (optional)")).toHaveValue("");
  }
  const field = screen.getByLabelText(
    input === "title" ? "Title, link or ID" : "Link or external ID (optional)",
  );
  expect(field).toHaveAttribute("maxlength", "2048");
  fireEvent.change(field, { target: { value: identifier } });
  const search = screen.getByRole("button", { name: "Search providers" });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalledWith({
    query: {
      kind: "movie",
      title: input === "title" ? identifier : "",
      year: null,
      external_ids: input === "external" ? [{ namespace: "", value: identifier }] : [],
      language: "en-US",
    },
    providers: ["tmdb"],
  }));
});

it("checks OMDb credentials before permitting a source search", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor = {
    ...snapshot.providers[0].descriptor,
    id: "omdb", name: "OMDb", fixture: false, requires_api_key: true,
  };
  snapshot.providers[0].config.enabled = true;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    provider_id: "omdb", supported_methods: ["api_key"], method: "api_key",
    storage: "system", configured: false, status: "not_configured",
  });
  show();
  await waitFor(() => expect(providerAuthApi.status).toHaveBeenCalledWith("omdb"));
  expect(screen.getByRole("checkbox", { name: /OMDb/ })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Search providers" })).toBeDisabled();
  expect(scraperApi.search).not.toHaveBeenCalled();
});
it.each([
  ["omdb", "OMDb", "movie"],
  ["douban", "Douban", "movie"],
  ["bangumi", "Bangumi", "anime"],
])("offers the enabled %s source for compatible manual searches", async (id, name, kind) => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor = {
    ...snapshot.providers[0].descriptor,
    id, name, fixture: false, media_kinds: [kind === "anime" ? "anime" : "movie"],
    requires_api_key: id === "omdb",
  };
  snapshot.providers[0].config.enabled = true;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  show();
  if (kind === "anime") chooseSelectOption(await screen.findByLabelText("Media type"), "anime");
  await screen.findByText(name);
  const checkbox = screen.getByRole("checkbox", { name: new RegExp(name) });
  await waitFor(() => expect(checkbox).toBeEnabled());
  fireEvent.click(checkbox);
  fireEvent.change(screen.getByLabelText("Title, link or ID"), { target: { value: "Source lookup" } });
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalledWith(expect.objectContaining({
    providers: [id], query: expect.objectContaining({ kind, title: "Source lookup" }),
  })));
});
it("reviews corrected IDs and stale metadata removals before allowing an identity-only confirmation", async () => {
  const stale = {
    id: "stale-overview", field_key: "overview", locale: "en-US",
    value: "Wrong film summary", provider_id: "tmdb", source_url: null,
    is_locked: false, is_user_modified: false,
  };
  vi.mocked(libraryApi.toolsDetail).mockResolvedValue({ ...localDetail(), metadata_values: [stale] });
  vi.mocked(scraperApi.detail).mockResolvedValue({ ...syntheticMetadata, data: { ...syntheticMetadata.data, fields: [] } });
  vi.mocked(scraperApi.preview).mockResolvedValue({
    ...syntheticMerge,
    fields: [{ key: "overview", language: "en-US", old: stale,
      new: { key: "overview", language: "en-US", value: null },
      provider_id: "tmdb", source_url: syntheticMetadata.data.identity.source_url,
      protected: false, needs_review: false, selected: true }],
    external_ids: [
      { namespace: "imdb", old: ["tt0000001"], new: ["tt35538033"] },
      { namespace: "douban", old: ["old-douban-id"], new: [] },
      { namespace: "tmdb", old: [], new: ["new-tmdb-id"] },
      { namespace: "wikidata", old: ["Q123"], new: ["Q123"] },
    ],
  });
  await searchAndChoose();
  await waitForMergeReady();
  expect(scraperApi.preview).toHaveBeenCalledWith(expect.objectContaining({ selected_fields: [], selected_artwork: [] }));
  const table = screen.getByRole("table");
  for (const namespace of ["imdb", "douban", "tmdb"]) expect(within(table).getByText(namespace)).toBeInTheDocument();
  expect(within(table).queryByText("wikidata")).not.toBeInTheDocument();
  const imdb = within(table).getByText("imdb").closest("tr")!;
  expect(within(imdb).getByText("tt0000001")).toBeInTheDocument();
  expect(within(imdb).getByText("tt35538033")).toBeInTheDocument();
  const removal = within(table).getByText("old-douban-id").closest("tr")!;
  expect(within(removal).getByText("Removed")).toBeInTheDocument();
  const staleRow = within(table).getByText("Wrong film summary").closest("tr")!;
  expect(within(staleRow).getByText("Removed")).toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Confirm merge (4 changes)" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Confirm merge (4 changes)" }));
  await waitFor(() => expect(scraperApi.confirm).toHaveBeenCalledExactlyOnceWith("merge-1"));
});
it("localizes the workflow in Chinese and snapshots the query language", async () => {
  await i18n.changeLanguage("zh-CN");
  show();
  await screen.findByText("TMDB");
  expect(screen.getByText("刮削工作台")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("片名、链接或 ID"), {
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
  await waitFor(() =>
    expect(screen.getByRole("checkbox", { name: /^Title/ })).toBeDisabled(),
  );
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
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
  chooseSelectOption(screen.getByLabelText("Media type"), "tv");
  fireEvent.change(screen.getByLabelText("Title, link or ID"), {
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
  chooseSelectOption(screen.getByLabelText("Media type"), "anime");
  fireEvent.change(screen.getByLabelText("Title, link or ID"), {
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
  await waitFor(() =>
    expect(toast.info).toHaveBeenCalledWith(
      "Saved candidates restored. No new provider search was sent.",
    ),
  );
  expect(scraperApi.searchPreview).toHaveBeenCalledWith("saved-run");
  expect(scraperApi.search).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByRole("region", { name: "Merge media metadata" });
  expect(scraperApi.detail).toHaveBeenCalledWith("run-1", 0);
});
it("discards an abandoned output preview on component unmount without executing it", async () => {
  const view = await searchAndChoose();
  await waitFor(() =>
    expect(screen.getByRole("checkbox", { name: /^Title/ })).toBeDisabled(),
  );
  await confirmMerge();
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Metadata confirmed. 1 fields applied.",
    ),
  );
  await openOutputPreview();
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
    expect(screen.getByLabelText("Title, link or ID")).toHaveValue("Local film"),
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
    screen.queryByRole("region", { name: "Merge media metadata" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /^Confirm merge/ }),
  ).not.toBeInTheDocument();
  expect(scraperApi.preview).not.toHaveBeenCalled();
});
it("discards a sidecar preview that arrives after target context is left", async () => {
  const view = await searchAndChoose();
  await confirmMerge();
  let finish!: (value: typeof syntheticOutput) => void;
  vi.mocked(scraperOutputApi.preview).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  await openOutputPreview();
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
  await waitForMergeReady();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(
    screen.queryByRole("region", { name: "Merge media metadata" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /^Confirm merge/ }),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this candidate" }),
  );
  await confirmMerge();
  await openOutputPreview();
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
  ).toHaveAttribute(
    "href",
    "/settings?tab=auth&from=scraper&media=media-1&provider=tmdb",
  );
  expect(
    screen.getByRole("link", { name: "Configure providers" }),
  ).toHaveAttribute(
    "href",
    "/settings?tab=providers&from=scraper&media=media-1",
  );
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
  expect(screen.getByLabelText("Title, link or ID")).toHaveValue("");
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
  fireEvent.change(screen.getByLabelText("Title, link or ID"), {
    target: { value: "Synthetic article" },
  });
  expect(
    screen.getByRole("button", { name: "Search providers" }),
  ).toBeDisabled();
  expect(
    screen.queryByText("Wikipedia requires the exact article title."),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "Wikipedia" }));
  expect(screen.getByLabelText("Title, link or ID")).toHaveAccessibleDescription(
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

it("invalidates a live merge preview when the actual local target changes", async () => {
  const view = await searchAndChoose();
  await waitForMergeReady();
  view.changeTarget("media-2");
  await waitFor(() =>
    expect(libraryApi.toolsDetail).toHaveBeenCalledWith("media-2"),
  );
  expect(
    screen.queryByRole("button", { name: /^Confirm merge/ }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("region", { name: "Merge media metadata" }),
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
    screen.queryByRole("region", { name: "Merge media metadata" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.preview).not.toHaveBeenCalled();
});

async function chooseQueueCandidate(onNavigate = vi.fn()) {
  const view = show("media-1", ["media-1", "media-2"], onNavigate);
  onNavigate.mockImplementation(view.changeTarget);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Search providers" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  await screen.findByRole("region", { name: "Merge media metadata" });
  return view;
}
it("navigates the real queue boundaries without searching, choosing or writing automatically", async () => {
  const navigate = vi.fn();
  const view = show(undefined, ["media-1", "media-2"], navigate);
  navigate.mockImplementation(view.changeTarget);
  await screen.findByText("Review queue · Item 1 of 2");
  expect(screen.getByRole("button", { name: "Previous item" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Skip current item" }));
  await screen.findByText("Review queue · Item 2 of 2");
  expect(navigate).toHaveBeenCalledWith("media-2");
  expect(
    screen.queryByRole("button", { name: "Next item" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Skip current item" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("link", { name: "Return to media list" }),
  ).toHaveAttribute("href", "/tools");
  fireEvent.click(screen.getByRole("button", { name: "Previous item" }));
  await screen.findByText("Review queue · Item 1 of 2");
  expect(scraperApi.search).not.toHaveBeenCalled();
  expect(scraperApi.detail).not.toHaveBeenCalled();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
it("disables queue navigation during a mutation and ignores the cancelled item's late search", async () => {
  let finish!: (value: typeof syntheticSearch) => void;
  vi.mocked(scraperApi.search).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const navigate = vi.fn();
  show("media-1", ["media-1", "media-2"], navigate);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Search providers" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  expect(
    screen.queryByRole("button", { name: "Next item" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Skip current item" }),
  ).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Skip current item" }));
  expect(navigate).not.toHaveBeenCalled();
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Stop waiting" }));
  expect(
    screen.getByRole("button", { name: "Skip current item" }),
  ).toBeEnabled();
  await act(async () => {
    finish(syntheticSearch);
  });
  expect(
    screen.queryByRole("button", { name: "Choose this candidate" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
});
it("releases an unexecuted sidecar preview before leaving a queue item", async () => {
  await chooseQueueCandidate();
  await confirmMerge();
  await openOutputPreview();
  await screen.findByText("/synthetic/library/film.nfo");
  expect(
    screen.queryByRole("button", { name: "Skip current item" }),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Next item" }));
  await screen.findByText("Review queue · Item 2 of 2");
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledWith("output-1"),
  );
  expect(
    screen.queryByText("/synthetic/library/film.nfo"),
  ).not.toBeInTheDocument();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  expect(scraperApi.confirm).toHaveBeenCalledTimes(1);
});
it("discards a late output preview from a queue item after an external route change", async () => {
  let confirmMetadata!: (
    value: Awaited<ReturnType<typeof scraperApi.confirm>>,
  ) => void;
  vi.mocked(scraperApi.confirm).mockImplementation(
    () =>
      new Promise((resolve) => {
        confirmMetadata = resolve;
      }),
  );
  const view = await chooseQueueCandidate();
  await confirmMerge();
  expect(scraperApi.confirm).toHaveBeenCalledExactlyOnceWith("merge-1");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Skip current item" }),
    ).toBeDisabled(),
  );
  expect(
    screen.queryByRole("button", { name: "Preview NFO and poster output" }),
  ).not.toBeInTheDocument();
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  await act(async () => {
    confirmMetadata({
      schema_version: 1,
      preview_id: "merge-1",
      applied_fields: 1,
      already_applied: false,
    });
  });
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Metadata confirmed. 1 fields applied.",
    ),
  );
  let finish!: (value: typeof syntheticOutput) => void;
  vi.mocked(scraperOutputApi.preview).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const outputButton = screen.getByRole("button", {
    name: "Preview NFO and poster output",
  });
  expect(outputButton).toBeEnabled();
  await act(async () => {
    fireEvent.click(outputButton);
  });
  expect(scraperOutputApi.preview).toHaveBeenCalledExactlyOnceWith("media-1", undefined, "online");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Next item" })).toBeDisabled(),
  );
  view.changeTarget("media-2");
  await screen.findByText("Review queue · Item 2 of 2");
  await act(async () => {
    finish(syntheticOutput);
  });
  await waitFor(() =>
    expect(scraperOutputApi.discard).toHaveBeenCalledExactlyOnceWith(
      "output-1",
    ),
  );
  expect(
    screen.queryByText("/synthetic/library/film.nfo"),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).toHaveBeenCalledTimes(1);
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("preserves the whole review queue in both manual provider setup links", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "tmdb";
  snapshot.providers[0].config.enabled = false;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  show("media-2", ["media-1", "media-2"], vi.fn());
  await screen.findByRole("link", { name: "Configure providers" });
  for (const [name, tab] of [
    ["Configure providers", "providers"],
    ["Configure credentials", "auth"],
  ]) {
    const href = screen.getByRole("link", { name }).getAttribute("href") ?? "";
    const url = new URL(href, "https://example.test");
    expect(url.pathname).toBe("/settings");
    expect(url.searchParams.get("tab")).toBe(tab);
    expect(url.searchParams.get("from")).toBe("scraper");
    expect(url.searchParams.get("media")).toBe("media-2");
    expect(JSON.parse(url.searchParams.get("queue") ?? "null")).toEqual([
      "media-1",
      "media-2",
    ]);
  }
});

it("applies real field presets, invalidates old previews and preserves protected fields", async () => {
  const detail = {
    ...localDetail(),
    metadata_values: [
      {
        id: "manual-title",
        field_key: "title",
        locale: "en-US",
        value: "My manual title",
        provider_id: null,
        source_url: null,
        is_locked: true,
        is_user_modified: true,
      },
      {
        id: "overview-1",
        field_key: "overview",
        locale: "en-US",
        value: "Local summary",
        provider_id: null,
        source_url: null,
        is_locked: false,
        is_user_modified: false,
      },
    ],
  };
  vi.mocked(libraryApi.toolsDetail).mockResolvedValue(detail);
  vi.mocked(scraperApi.preview).mockImplementation(async (request) => ({
    ...syntheticMerge,
    fields: syntheticMerge.fields.map((field) => ({
      ...field,
      old:
        detail.metadata_values.find((value) => value.field_key === field.key) ??
        null,
      selected: request.selected_fields.includes(field.key) && !field.protected,
    })),
  }));
  await searchAndChoose();
  await waitForMergeReady();
  expect(screen.getByRole("checkbox", { name: /^Title/ })).toBeDisabled();
  expect(scraperApi.preview).toHaveBeenLastCalledWith(
    expect.objectContaining({ selected_fields: ["overview"] }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Deselect all fields" }));
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).not.toBeChecked();
  fireEvent.click(
    screen.getByRole("button", { name: "Select all unprotected fields" }),
  );
  await waitForMergeReady();
  expect(screen.getByRole("checkbox", { name: /^Title/ })).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).toBeChecked();
  fireEvent.click(
    screen.getByRole("button", { name: "Fill missing fields only" }),
  );
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).not.toBeChecked();
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  fireEvent.click(
    screen.getByRole("button", { name: "Select all unprotected fields" }),
  );
  await waitForMergeReady();
  expect(scraperApi.preview).toHaveBeenLastCalledWith(
    expect.objectContaining({ selected_fields: ["overview"] }),
  );
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});
it("updates the visible merged result immediately and requires new validation before confirming changed fields", async () => {
  vi.mocked(scraperApi.preview).mockImplementation(async (request) => ({
    ...syntheticMerge,
    media_item_id: request.media_item_id,
    fields: syntheticMerge.fields.map((field) => ({
      ...field,
      old: null,
      protected: false,
      selected: request.selected_fields.includes(field.key),
    })),
  }));
  await searchAndChoose();
  await waitForMergeReady();
  expect(
    within(screen.getByRole("tabpanel", { name: "Merged" })).getByText(
      "Synthetic provider summary",
    ),
  ).toBeVisible();
  let finish!: () => void;
  vi.mocked(scraperApi.preview).mockImplementationOnce(
    (request) =>
      new Promise((resolve) => {
        finish = () =>
          resolve({
            ...syntheticMerge,
            preview_id: "changed-fields",
            media_item_id: request.media_item_id,
            fields: syntheticMerge.fields.map((field) => ({
              ...field,
              old: null,
              protected: false,
              selected: request.selected_fields.includes(field.key),
            })),
          });
      }),
  );
  fireEvent.click(screen.getByRole("checkbox", { name: /^Overview/ }));
  expect(
    within(screen.getByRole("tabpanel", { name: "Merged" })).queryByText(
      "Synthetic provider summary",
    ),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: /^Confirm merge/ }));
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(scraperApi.preview).toHaveBeenLastCalledWith(
      expect.objectContaining({ selected_fields: ["title"] }),
    ),
  );
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  await act(async () => finish());
  await waitForMergeReady();
  await confirmMerge();
  await waitFor(() =>
    expect(scraperApi.confirm).toHaveBeenCalledWith("changed-fields"),
  );
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("only fills missing fields in the live result and the validated request", async () => {
  await searchAndChoose();
  await waitForMergeReady();
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Only fill missing fields and artwork",
    }),
  );
  expect(screen.getByRole("checkbox", { name: /^Title/ })).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: /^Title/ })).not.toBeChecked();
  expect(
    within(screen.getByRole("tabpanel", { name: "Merged" })).getByRole(
      "heading",
      { name: "Local film" },
    ),
  ).toBeVisible();
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  await waitForMergeReady();
  expect(scraperApi.preview).toHaveBeenLastCalledWith(
    expect.objectContaining({
      selected_fields: ["overview"],
      selected_artwork: [],
    }),
  );
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).toBeChecked();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("keeps the last-item return action disabled while a mutation is executing", async () => {
  let finish!: (value: typeof syntheticSearch) => void;
  vi.mocked(scraperApi.search).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  show("media-2", ["media-1", "media-2"], vi.fn());
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Search providers" }),
    ).toBeEnabled(),
  );
  expect(
    screen.getByRole("link", { name: "Return to media list" }),
  ).toHaveAttribute("href", "/tools");
  fireEvent.click(screen.getByRole("button", { name: "Search providers" }));
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalled());
  expect(
    screen.queryByRole("link", { name: "Return to media list" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Return to media list" }),
  ).toBeDisabled();
  await act(async () => {
    finish(syntheticSearch);
  });
  await screen.findByRole("link", { name: "Return to media list" });
});
it("ignores a search result even when cancellation happens before the mutation function starts", async () => {
  let finish!: (value: typeof syntheticSearch) => void;
  vi.mocked(scraperApi.search).mockImplementation(
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
  fireEvent.click(screen.getByRole("button", { name: "Stop waiting" }));
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalled());
  await act(async () => {
    finish(syntheticSearch);
  });
  expect(
    screen.queryByRole("button", { name: "Choose this candidate" }),
  ).not.toBeInTheDocument();
});

const savedSessionId = "11111111-1111-4111-8111-111111111111";
function storedReview(
  state: ReviewQueue["items"][number]["state"] = "pending",
): ReviewQueue {
  return {
    id: savedSessionId,
    revision: 0,
    current_media_id: state === "pending" ? "media-1" : null,
    context: {
      q: "Local",
      kind: "movie",
      health: "incomplete_metadata",
      sort: "title_asc",
      cursor: "fixture-cursor",
      page: 2,
    },
    items: [
      {
        media_id: "media-1",
        position: 0,
        state,
        checkpoint: null,
        preview_id: null,
        needs_recheck: false,
        reason: null,
      },
    ],
    created_at: "2026-11-04T10:00:00Z",
    updated_at: "2026-11-04T10:00:00Z",
  };
}
function persistQueue(initial = storedReview()) {
  let saved = initial;
  vi.mocked(reviewQueueApi.get).mockImplementation(async () =>
    structuredClone(saved),
  );
  vi.mocked(reviewQueueApi.checkpoint).mockImplementation(async (input) => {
    saved = {
      ...saved,
      revision: saved.revision + 1,
      items: saved.items.map((item) => ({
        ...item,
        checkpoint: "proof-1",
        preview_id: input.preview_id,
      })),
    };
    return structuredClone(saved);
  });
  vi.mocked(reviewQueueApi.update).mockImplementation(async ({ action }) => {
    const state =
      action === "defer"
        ? "deferred"
        : action === "failed"
          ? "failed"
          : action === "confirmed"
            ? "confirmed"
            : "pending";
    saved = {
      ...saved,
      revision: saved.revision + 1,
      current_media_id: state === "pending" ? "media-1" : null,
      items: saved.items.map((item) => ({ ...item, state })),
    };
    return structuredClone(saved);
  });
}
async function showSavedQueue(state?: ReviewQueue["items"][number]["state"]) {
  persistQueue(storedReview(state));
  const view = show("media-1", undefined, vi.fn(), savedSessionId);
  await screen.findByRole("region", { name: "Saved review queues" });
  return view;
}

it("restores a durable session without searching and returns the original tools filters and page", async () => {
  await showSavedQueue();
  await screen.findByDisplayValue("Local film");
  const link = screen.getByRole("link", { name: "Return to media list" });
  const query = new URL(
    link.getAttribute("href") ?? "",
    "https://fixture.invalid",
  ).searchParams;
  expect(Object.fromEntries(query)).toMatchObject({
    q: "Local",
    kind: "movie",
    health: "incomplete_metadata",
    sort: "title_asc",
    cursor: "fixture-cursor",
    page: "2",
  });
  expect(scraperApi.search).not.toHaveBeenCalled();
  expect(reviewQueueApi.update).not.toHaveBeenCalled();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("persists deferral of the last item and requires explicit review after reopening", async () => {
  const view = await showSavedQueue();
  fireEvent.click(screen.getByRole("button", { name: "Defer this item" }));
  await screen.findByText(
    "This item was deferred. Review it again to continue.",
  );
  expect(reviewQueueApi.update).toHaveBeenCalledWith({
    queue_id: savedSessionId,
    media_id: "media-1",
    expected_revision: 0,
    action: "defer",
  });
  view.unmount();
  show("media-1", undefined, vi.fn(), savedSessionId);
  fireEvent.click(
    await screen.findByRole("button", { name: "Review this item again" }),
  );
  await screen.findByDisplayValue("Local film");
  expect(reviewQueueApi.update).toHaveBeenLastCalledWith({
    queue_id: savedSessionId,
    media_id: "media-1",
    expected_revision: 1,
    action: "retry",
  });
  expect(scraperApi.search).not.toHaveBeenCalled();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
});

it("keeps a failed queued search's specific recovery visible and persists explicit retry before searching again", async () => {
  await showSavedQueue();
  vi.mocked(scraperApi.search).mockResolvedValueOnce({
    ...syntheticSearch,
    candidates: [],
    failures: [{ provider_id: "tmdb", failure: "timeout" }],
  });
  const search = await screen.findByRole("button", {
    name: "Search providers",
  });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  await screen.findByText(
    "This item failed. Review it again to retry the search; metadata and files are never applied automatically.",
  );
  expect(
    screen.getByText(
      "The source request timed out. You can search this item again.",
    ),
  ).toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Search this item again" }),
  );
  await waitFor(() => expect(scraperApi.search).toHaveBeenCalledTimes(2));
  expect(reviewQueueApi.update).toHaveBeenLastCalledWith(
    expect.objectContaining({ action: "retry", media_id: "media-1" }),
  );
  expect(
    vi.mocked(reviewQueueApi.update).mock.invocationCallOrder.at(-1),
  ).toBeLessThan(
    vi.mocked(scraperApi.search).mock.invocationCallOrder.at(-1) ?? 0,
  );
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("binds a durable checkpoint before explicit confirmation and leaves sidecar output separate", async () => {
  await showSavedQueue();
  const search = await screen.findByRole("button", {
    name: "Search providers",
  });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  await confirmMerge();
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Metadata confirmed. 1 fields applied.",
    ),
  );
  expect(reviewQueueApi.checkpoint).toHaveBeenCalledWith({
    queue_id: savedSessionId,
    media_id: "media-1",
    expected_revision: 0,
    preview_id: "merge-1",
  });
  expect(reviewQueueApi.update).toHaveBeenLastCalledWith({
    queue_id: savedSessionId,
    media_id: "media-1",
    expected_revision: 1,
    action: "confirmed",
    checkpoint: "proof-1",
  });
  expect(
    vi.mocked(reviewQueueApi.checkpoint).mock.invocationCallOrder[0],
  ).toBeLessThan(vi.mocked(scraperApi.confirm).mock.invocationCallOrder[0]);
  expect(
    vi.mocked(scraperApi.confirm).mock.invocationCallOrder[0],
  ).toBeLessThan(vi.mocked(reviewQueueApi.update).mock.invocationCallOrder[0]);
  expect(
    screen.getByRole("button", { name: "Preview NFO and poster output" }),
  ).toBeEnabled();
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("does not replay metadata confirmation or sidecar writes when a confirmed queue is restored", async () => {
  await showSavedQueue("confirmed");
  expect(
    screen.getByText("There are no pending items in this queue."),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Search providers" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(reviewQueueApi.checkpoint).not.toHaveBeenCalled();
  expect(reviewQueueApi.update).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("blocks unavailable queue items while retaining a safe return and refresh action", async () => {
  const queue = storedReview();
  queue.items[0] = {
    ...queue.items[0],
    needs_recheck: true,
    reason: "unavailable_media",
  };
  persistQueue(queue);
  show("media-1", undefined, vi.fn(), savedSessionId);
  await screen.findByText(
    "This media was removed or its folder is unavailable. Restore the folder and refresh the queue.",
  );
  expect(screen.getByRole("button", { name: "Refresh queue" })).toBeEnabled();
  expect(
    screen.queryByRole("button", { name: "Search providers" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Review this item again" }),
  ).not.toBeInTheDocument();
  expect(scraperApi.search).not.toHaveBeenCalled();
});

it("refuses confirmation after a checkpoint conflict and never marks the item confirmed", async () => {
  await showSavedQueue();
  vi.mocked(reviewQueueApi.checkpoint).mockRejectedValue(
    new DesktopError("request_failed", {
      schema_version: 1,
      code: "conflict",
      message: "safe",
      correlation_id: "fixture",
    }),
  );
  const search = await screen.findByRole("button", {
    name: "Search providers",
  });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  await confirmMerge();
  await screen.findByText(
    "This item failed. Review it again to retry the search; metadata and files are never applied automatically.",
  );
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(
    vi
      .mocked(reviewQueueApi.update)
      .mock.calls.some(([input]) => input.action === "confirmed"),
  ).toBe(false);
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("preserves the durable session and current media in both source configuration links", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "tmdb";
  snapshot.providers[0].descriptor.media_kinds = ["movie", "tv_series"];
  snapshot.providers[0].config.enabled = false;
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  await showSavedQueue();
  for (const name of ["Configure providers", "Configure credentials"]) {
    const link = await screen.findByRole("link", { name });
    const url = new URL(
      link.getAttribute("href") ?? "",
      "https://fixture.invalid",
    );
    expect(url.pathname).toBe("/settings");
    expect(url.searchParams.get("session")).toBe(savedSessionId);
    expect(url.searchParams.get("media")).toBe("media-1");
    expect(url.searchParams.get("queue")).toBeNull();
  }
});

it("retains the durable session when a failed source response sends the user to credential setup", async () => {
  await showSavedQueue();
  vi.mocked(scraperApi.search).mockResolvedValueOnce({
    ...syntheticSearch,
    candidates: [],
    failures: [{ provider_id: "tmdb", failure: "credentials_unavailable" }],
  });
  const search = await screen.findByRole("button", {
    name: "Search providers",
  });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  await screen.findByText(
    "This item failed. Review it again to retry the search; metadata and files are never applied automatically.",
  );
  const configure = await screen.findByRole("button", {
    name: "Configure source and return · TMDB",
  });
  await waitFor(() => expect(configure).toBeEnabled());
  fireEvent.click(configure);
  expect(recoveryNavigate).toHaveBeenCalledWith({
    to: "/settings",
    search: {
      tab: "auth",
      from: "scraper",
      session: savedSessionId,
      media: "media-1",
      provider: "tmdb",
    },
  });
  expect(scraperApi.confirm).not.toHaveBeenCalled();
});

it("does not apply a checkpoint response that arrives after leaving the queue", async () => {
  const view = await showSavedQueue();
  let finish: (value: ReviewQueue) => void = () => {};
  vi.mocked(reviewQueueApi.checkpoint).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const search = await screen.findByRole("button", {
    name: "Search providers",
  });
  await waitFor(() => expect(search).toBeEnabled());
  fireEvent.click(search);
  fireEvent.click(
    await screen.findByRole("button", { name: "Choose this candidate" }),
  );
  await confirmMerge();
  await waitFor(() => expect(reviewQueueApi.checkpoint).toHaveBeenCalled());
  view.unmount();
  await act(async () => {
    finish({
      ...storedReview(),
      revision: 1,
      items: [
        {
          ...storedReview().items[0],
          checkpoint: "proof-1",
          preview_id: "merge-1",
        },
      ],
    });
  });
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  expect(
    vi
      .mocked(reviewQueueApi.update)
      .mock.calls.some(([input]) => input.action === "confirmed"),
  ).toBe(false);
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("refuses a concurrent only-missing overwrite and refreshes to a proof containing only the remaining empty field", async () => {
  const genre = { key: "genres", value: ["Drama"], language: null };
  if (syntheticMetadata.kind !== "movie")
    throw new Error("Expected movie fixture");
  const remoteFields = [...syntheticMetadata.data.fields, genre];
  vi.mocked(scraperApi.detail).mockResolvedValue({
    ...syntheticMetadata,
    data: { ...syntheticMetadata.data, fields: remoteFields },
  });
  let filledConcurrently = false;
  const filledOverview = {
    id: "concurrent-overview",
    field_key: "overview",
    locale: "en-US",
    value: "Filled concurrently",
    provider_id: "tmdb",
    source_url: syntheticMetadata.data.identity.source_url,
    is_locked: false,
    is_user_modified: false,
  };
  vi.mocked(libraryApi.toolsDetail).mockImplementation(async (id) => ({
    ...localDetail(id),
    metadata_values: filledConcurrently ? [filledOverview] : [],
  }));
  vi.mocked(scraperApi.preview).mockImplementation(async (request) => ({
    ...syntheticMerge,
    preview_id: `proof-${request.selected_fields.join("-")}`,
    media_item_id: request.media_item_id,
    fields: remoteFields.map((field) => ({
      key: field.key,
      language: field.language,
      old:
        filledConcurrently && field.key === "overview" ? filledOverview : null,
      new: field,
      provider_id: "tmdb",
      source_url: syntheticMetadata.data.identity.source_url,
      protected: false,
      needs_review: filledConcurrently && field.key === "overview",
      selected: request.selected_fields.includes(field.key),
    })),
  }));
  await searchAndChoose();
  await waitForMergeReady();
  filledConcurrently = true;
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Only fill missing fields and artwork",
    }),
  );
  await screen.findByText(
    "Local metadata has changed. Refresh the preview and review it again.",
  );
  expect(scraperApi.preview).toHaveBeenLastCalledWith(
    expect.objectContaining({ selected_fields: ["overview", "genres"] }),
  );
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  expect(
    screen.getByText("1 changes (1 added, 0 replaced, 0 artwork, 2 skipped)"),
  ).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: /^Confirm merge/ }));
  expect(scraperApi.confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Refresh preview" }));
  await waitForMergeReady();
  expect(scraperApi.preview).toHaveBeenLastCalledWith(
    expect.objectContaining({ selected_fields: ["genres"] }),
  );
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: /^Overview/ })).toBeDisabled();
  expect(
    within(screen.getByRole("tabpanel", { name: "Merged" })).getByText(
      "Filled concurrently",
    ),
  ).toBeVisible();
  await confirmMerge();
  await waitFor(() =>
    expect(scraperApi.confirm).toHaveBeenCalledWith("proof-genres"),
  );
  expect(scraperOutputApi.preview).not.toHaveBeenCalled();
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("confirms exactly the current proof when clicked in its first enabled commit", async () => {
  let finish!: (value: typeof syntheticMerge) => void;
  vi.mocked(scraperApi.preview).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  await searchAndChoose();
  await waitFor(() => expect(scraperApi.preview).toHaveBeenCalled());
  const click = clickFirstEnabledCommit(/^Confirm merge/);
  try {
    // Do not pre-flush passive effects with act before the observer can click.
    finish(syntheticMerge);
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        "Metadata confirmed. 1 fields applied.",
      ),
    );
    expect(click.clicks()).toBe(1);
    expect(scraperApi.confirm).toHaveBeenCalledExactlyOnceWith("merge-1");
    expect(scraperOutputApi.preview).not.toHaveBeenCalled();
    expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  } finally {
    click.observer.disconnect();
  }
});

it.each(["execute", "discard"] as const)(
  "uses exactly the visible output proof for %s in its first enabled commit",
  async (action) => {
    let finish!: (value: typeof syntheticOutput) => void;
    vi.mocked(scraperOutputApi.preview).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await searchAndChoose();
    await confirmMerge();
    await openOutputPreview();
    await waitFor(() =>
      expect(scraperOutputApi.preview).toHaveBeenCalledExactlyOnceWith(
        "media-1",
        undefined,
        "online",
      ),
    );
    const click = clickFirstEnabledCommit(
      action === "execute"
        ? "Confirm and write sidecars"
        : "Cancel output preview",
    );
    try {
      finish(syntheticOutput);
      if (action === "execute") {
        await waitFor(() =>
          expect(toast.success).toHaveBeenCalledWith(
            "Output recorded: 1 written, 0 preserved.",
          ),
        );
      } else {
        await waitFor(() =>
          expect(scraperOutputApi.discard).toHaveBeenCalledWith("output-1"),
        );
        await waitFor(() =>
          expect(
            screen.queryByText("/synthetic/library/film.nfo"),
          ).not.toBeInTheDocument(),
        );
      }
      // Settle lifecycle cleanup as well as the direct mutation before counting.
      await act(async () => {});
      expect(click.clicks()).toBe(1);
      expect(scraperOutputApi[action]).toHaveBeenCalledExactlyOnceWith(
        "output-1",
      );
      if (action === "discard")
        expect(scraperOutputApi.execute).not.toHaveBeenCalled();
      expect(scraperApi.confirm).toHaveBeenCalledExactlyOnceWith("merge-1");
    } finally {
      click.observer.disconnect();
    }
  },
);
