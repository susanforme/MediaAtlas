import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useState } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import {
  libraryApi,
  type MediaDetail,
  type MediaFile,
} from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import i18n from "../../i18n";
import { LibraryPage } from "./browse";
import { MediaDetailPage } from "./detail";
import { NativeOpen } from "./native-open";

vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/organization", async (original) => ({
  ...(await original<typeof import("../../api/organization")>()),
  nativeOpenApi: { open: vi.fn() },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    detail: vi.fn(),
    media: vi.fn(),
    roots: vi.fn(),
    facets: vi.fn(),
  },
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { getMedia: vi.fn() },
}));
const file = (
  id: string,
  path: string,
  part: number | null = null,
): MediaFile => ({
  id,
  media_item_id: "media",
  root_id: "root",
  relative_path: path,
  size_bytes: 2,
  modified_at: null,
  part,
});
const detail: MediaDetail = {
  media: {
    id: "media",
    kind: "tv_episode",
    title: "Confirmed resource",
    original_title: null,
    year: 2026,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  },
  files: [],
  artworks: [],
  external_ids: [],
  metadata_values: [],
  relations: [],
  health: [],
  poster_uri: null,
};
function show(node: React.ReactNode) {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: {
              queries: { retry: false },
              mutations: { retry: false },
            },
          })
        }
      >
        {node}
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
function ChangingResource() {
  const [id, setId] = useState("first");
  return (
    <>
      <button type="button" onClick={() => setId("second")}>
        Change resource
      </button>
      <NativeOpen mediaId={id} />
    </>
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  vi.mocked(nativeOpenApi.open).mockResolvedValue({ requested: true });
  vi.mocked(libraryApi.detail).mockResolvedValue({
    ...detail,
    files: [file("movie", "feature.mkv")],
  });
  vi.mocked(adultApi.getMedia).mockResolvedValue({
    ...detail,
    files: [file("adult-file", "feature.mp4")],
  });
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.facets).mockResolvedValue({
    genres: [],
    people: [],
    studios: [],
    years: [],
  });
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [{ media: detail.media, poster_uri: null, health: [] }],
    next_cursor: null,
  });
});
it("opens one explicitly clicked file ID and reports only system request submission", async () => {
  show(<NativeOpen mediaId="media" />);
  expect(libraryApi.detail).not.toHaveBeenCalled();
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await screen.findByText("Submitted system opening request");
  expect(nativeOpenApi.open).toHaveBeenCalledWith("movie", false);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});
it("requires an explicit version or Part selection and excludes sidecars", async () => {
  show(
    <NativeOpen
      files={[
        file("part1", "feature.Part1.mkv", 1),
        file("part2", "feature.Part2.mkv", 2),
        file("nfo", "feature.nfo"),
        file("script", "run.exe"),
      ]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await screen.findByRole("dialog", {
    name: "Choose a version or Part to open",
  });
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  expect(screen.queryByText("feature.nfo")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /feature.Part2.mkv/ }));
  await waitFor(() =>
    expect(nativeOpenApi.open).toHaveBeenCalledWith("part2", false),
  );
  expect(nativeOpenApi.open).toHaveBeenCalledOnce();
});
it("uses the adult typed endpoint without ordinary detail queries", async () => {
  show(<NativeOpen mediaId="private" adult />);
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await screen.findByText("Submitted system opening request");
  expect(adultApi.getMedia).toHaveBeenCalledWith("private");
  expect(libraryApi.detail).not.toHaveBeenCalled();
  expect(nativeOpenApi.open).toHaveBeenCalledWith("adult-file", true);
});
it("keeps library cards and details read-only with a primary native open action", async () => {
  const view = show(<LibraryPage search={{}} onSearch={() => {}} />);
  const primary = await screen.findByRole("button", {
    name: "Open in system app: Confirmed resource",
  });
  expect(screen.getByRole("link", { name: "Details" })).toBeInTheDocument();
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  fireEvent.click(primary);
  await screen.findByText("Submitted system opening request");
  view.unmount();
  const page = show(<MediaDetailPage mediaId="media" />);
  await screen.findByRole("heading", { name: "Confirmed resource" });
  expect(
    screen.queryByRole("button", { name: /Preview|Confirm|Export|Scrape/ }),
  ).not.toBeInTheDocument();
  expect(page.container.querySelector("audio,video")).toBeNull();
  expect(screen.getByRole("link", { name: "Media tools" })).toBeInTheDocument();
});

it("does not dispatch a delayed card-detail response after the view closes", async () => {
  let resolve!: (value: MediaDetail) => void;
  vi.mocked(libraryApi.detail).mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  const view = show(<NativeOpen mediaId="media" />);
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await waitFor(() => expect(libraryApi.detail).toHaveBeenCalled());
  view.unmount();
  resolve({ ...detail, files: [file("late", "late.mkv")] });
  await new Promise((r) => setTimeout(r, 10));
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
});

it("invalidates a pending file selection synchronously when the displayed resource changes", async () => {
  let resolve!: (value: MediaDetail) => void;
  vi.mocked(libraryApi.detail).mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  show(<ChangingResource />);
  fireEvent.click(screen.getByRole("button", { name: "Open in system app" }));
  await waitFor(() =>
    expect(libraryApi.detail).toHaveBeenCalledWith("first", false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Change resource" }));
  await act(async () =>
    resolve({ ...detail, files: [file("old-file", "old.mkv")] }),
  );
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("traps keyboard focus in version selection and restores the opening button on Escape", async () => {
  show(<NativeOpen files={[file("one", "one.mkv"), file("two", "two.mkv")]} />);
  const trigger = screen.getByRole("button", { name: "Open in system app" });
  trigger.focus();
  fireEvent.click(trigger);
  const dialog = await screen.findByRole("dialog", {
    name: "Choose a version or Part to open",
  });
  await waitFor(() =>
    expect(dialog).toContainElement(document.activeElement as HTMLElement),
  );
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(trigger).toHaveFocus();
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
});

it("mounts the single library search in the toolbar before data arrives and preserves its focus", async () => {
  let resolve!: (value: { items: []; next_cursor: null }) => void;
  vi.mocked(libraryApi.media).mockReturnValue(
    new Promise((done) => {
      resolve = done;
    }),
  );
  const onSearch = vi.fn();
  show(
    <>
      <div id="library-toolbar-search" />
      <LibraryPage
        search={{ q: "Initial query", cursor: "old-cursor", page: 4 }}
        onSearch={onSearch}
      />
    </>,
  );
  const input = screen.getByRole("searchbox");
  expect(document.getElementById("library-toolbar-search")).toContainElement(
    input,
  );
  expect(screen.getAllByRole("searchbox")).toHaveLength(1);
  input.focus();
  await act(async () => resolve({ items: [], next_cursor: null }));
  expect(screen.getByRole("searchbox")).toBe(input);
  expect(input).toHaveFocus();
  fireEvent.change(input, { target: { value: "Changed query" } });
  expect(onSearch).toHaveBeenCalledWith({
    q: "Changed query",
    cursor: undefined,
    page: 1,
  });
});
