import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { type ComponentProps, useState } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { libraryApi, type MediaCard, type RootView } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import i18n from "../../i18n";
import { LibraryPage, type LibrarySearch } from "./browse";

vi.mock("./context", () => ({
  usePrivacy: () => ({
    settings: { entry_visible: false, covers_visible: false },
    includeAdult: false,
  }),
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    params,
    children,
    onClick,
    ...props
  }: ComponentProps<"a"> & {
    to: string;
    search?: Record<string, unknown>;
    params?: { mediaId: string };
  }) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).map(([key, value]) => [key, String(value)]),
    ).toString();
    return (
      <a
        {...props}
        href={`${to.replace("$mediaId", params?.mediaId ?? "")}${query ? `?${query}` : ""}`}
        onClick={(event) => {
          event.preventDefault();
          onClick?.(event);
        }}
      >
        {children}
      </a>
    );
  },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    roots: vi.fn(),
    media: vi.fn(),
    facets: vi.fn(),
    tasks: vi.fn(),
    detail: vi.fn(),
  },
}));
vi.mock("../../api/organization", async (original) => ({
  ...(await original<typeof import("../../api/organization")>()),
  nativeOpenApi: { open: vi.fn() },
}));
const card: MediaCard = {
  media: {
    id: "synthetic-film",
    kind: "movie",
    title: "Synthetic film",
    original_title: null,
    year: 2026,
    created_at: "2026-11-01",
    updated_at: "2026-11-01",
  },
  poster_uri: "data:image/png;base64,YQ==",
  health: ["metadata_needs_review", "missing_nfo"],
};
const root: RootView = {
  id: "synthetic-folder",
  name: "Synthetic folder",
  path: "/synthetic/media",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
  created_at: "2026-11-01",
  updated_at: "2026-11-01",
  last_scan_at: null,
};
function ControlledLibrary({
  initial,
  onSearch,
}: {
  initial: LibrarySearch;
  onSearch: (next: LibrarySearch, replace?: boolean) => void;
}) {
  const [search, setSearch] = useState(initial);
  return (
    <LibraryPage
      search={search}
      onSearch={(next, replace) => {
        onSearch(next, replace);
        setSearch(next);
      }}
    />
  );
}
function show(initial: LibrarySearch = {}) {
  const onSearch = vi.fn();
  const view = render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <ControlledLibrary initial={initial} onSearch={onSearch} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  return { ...view, onSearch };
}
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([root]);
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [card],
    next_cursor: null,
  });
  vi.mocked(libraryApi.facets).mockResolvedValue({
    genres: [{ value: "Drama", count: 1 }],
    people: [],
    studios: [],
    years: [{ value: "2026", count: 1 }],
  });
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
});
it("defaults to a keyboard-focusable full poster details link without launching an external app", async () => {
  show();
  const poster = await screen.findByRole("link", {
    name: "Details: Synthetic film",
  });
  expect(poster).toHaveAttribute("href", "/media/synthetic-film");
  expect(poster).toHaveAccessibleDescription("2026 · Movie Match needs review");
  expect(
    within(poster).getByRole("img", { name: card.media.title }),
  ).toBeInTheDocument();
  expect(within(poster).getByText("2026 · Movie")).toBeInTheDocument();
  expect(within(poster).getByText("Match needs review")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Poster wall" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Open in system app/ }),
  ).not.toBeInTheDocument();
  poster.focus();
  expect(poster).toHaveFocus();
  fireEvent.click(poster);
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  expect(libraryApi.detail).not.toHaveBeenCalled();
});
it("keeps detailed filters collapsed while preserving server filtering and resetting an obsolete cursor", async () => {
  const { onSearch } = show({ page: 4, cursor: "synthetic-old-cursor" });
  await screen.findByRole("link", { name: "Details: Synthetic film" });
  const toggle = screen.getByRole("button", { name: "Filters" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(
    screen.getByRole("combobox", { name: "Media type" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Sort" })).toBeInTheDocument();
  expect(
    screen.queryByRole("combobox", { name: "Library root" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("combobox", { name: "Health" }),
  ).not.toBeInTheDocument();
  fireEvent.click(toggle);
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(
    screen.getByRole("region", { name: "Advanced filters" }),
  ).toBeInTheDocument();
  fireEvent.change(screen.getByRole("combobox", { name: "Library root" }), {
    target: { value: root.id },
  });
  expect(onSearch).toHaveBeenCalledWith(
    { root: root.id, cursor: undefined, page: 1 },
    undefined,
  );
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        root_id: root.id,
        cursor: null,
        include_adult: false,
      }),
    ),
  );
  await screen.findByRole("option", { name: "Drama (1)" });
  fireEvent.change(screen.getByRole("combobox", { name: "All genres" }), {
    target: { value: "Drama" },
  });
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        root_id: root.id,
        genre: "Drama",
        cursor: null,
      }),
    ),
  );
  fireEvent.click(toggle);
  expect(
    screen.queryByRole("region", { name: "Advanced filters" }),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({ root_id: null, genre: null, cursor: null }),
    ),
  );
});
it("keeps the poster selectable after cached artwork fails", async () => {
  show();
  const poster = await screen.findByRole("link", {
    name: "Details: Synthetic film",
  });
  fireEvent.error(within(poster).getByRole("img"));
  expect(within(poster).queryByRole("img")).not.toBeInTheDocument();
  expect(within(poster).getByText("No poster")).toBeInTheDocument();
  expect(poster).toHaveAttribute("href", "/media/synthetic-film");
});
it("opens directory creation directly from the empty library", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  show();
  await screen.findByText("Your library is empty");
  for (const link of screen.getAllByRole("link", { name: "Add root" }))
    expect(link).toHaveAttribute("href", "/roots?add=true");
  expect(
    screen.queryByRole("button", { name: "Previous" }),
  ).not.toBeInTheDocument();
});
it("keeps explicit list view and recovers a failed media request without fake cards", async () => {
  vi.mocked(libraryApi.media).mockRejectedValueOnce(
    new DesktopError("request_failed"),
  );
  show({ view: "list" });
  expect(await screen.findByRole("alert")).toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Details: Synthetic film" }),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Check again" }));
  expect(await screen.findByRole("table")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "List" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
});

it("offers source recovery for persisted unconfigured cards even without task history", async () => {
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [{ ...card, health: ["metadata_unconfigured"] }],
    next_cursor: null,
  });
  show();
  expect(
    await screen.findByRole("link", { name: "Configure source and continue" }),
  ).toHaveAttribute("href", "/settings?from=library&root=synthetic-folder");
  expect(libraryApi.detail).not.toHaveBeenCalled();
});

it("does not promise a folder retry when multiple folders leave the source scope unknown", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    root,
    { ...root, id: "another-folder" },
  ]);
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [{ ...card, health: ["metadata_unconfigured"] }],
    next_cursor: null,
  });
  show();
  expect(
    await screen.findByRole("link", { name: "Configure source" }),
  ).toHaveAttribute("href", "/settings?from=library");
  expect(
    screen.queryByRole("link", { name: "Configure source and continue" }),
  ).not.toBeInTheDocument();
});

it("retains the actual folder and type filter for source recovery", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    root,
    { ...root, id: "another-folder" },
  ]);
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [{ ...card, health: ["metadata_unconfigured"] }],
    next_cursor: null,
  });
  show({ root: "another-folder", kind: "movie" });
  expect(
    await screen.findByRole("link", { name: "Configure source and continue" }),
  ).toHaveAttribute(
    "href",
    "/settings?from=library&root=another-folder&kind=movie",
  );
});
