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
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { libraryApi, type MediaCard, type RootView } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import i18n from "../../i18n";
import { chooseSelectOption, selectOptions } from "../../test/select";
import { LibraryPage, type LibrarySearch } from "./browse";

const privacy = vi.hoisted(() => ({
  settings: { entry_visible: false, covers_visible: false },
  includeAdult: false,
}));
vi.mock("./context", () => ({ usePrivacy: () => privacy }));
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
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { media: vi.fn(), roots: vi.fn() },
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
      adult={search.kind === "adult_video"}
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
  privacy.settings.entry_visible = false;
  privacy.includeAdult = false;
  vi.mocked(adultApi.roots).mockResolvedValue([
    { ...root, kind: "adult_video" },
  ]);
  vi.mocked(adultApi.media).mockResolvedValue({
    items: [{ ...card, media: { ...card.media, kind: "adult_video" } }],
    next_cursor: null,
  });
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
  chooseSelectOption(screen.getByRole("combobox", { name: "Library root" }), root.id);
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
  selectOptions(screen.getByRole("combobox", { name: "All genres" }));
  await screen.findByRole("option", { name: "Drama (1)" });
  chooseSelectOption(screen.getByRole("combobox", { name: "All genres" }), "Drama");
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

it("uses server health shortcuts while retaining search scope and clearing only health for All", async () => {
  const initial: LibrarySearch = {
    root: root.id,
    kind: "movie",
    q: "Scoped search",
    sort: "year_desc",
    genre: "Drama",
    cursor: "old-page",
    page: 8,
    view: "wall",
  };
  vi.mocked(libraryApi.media).mockImplementation(async (query) => ({
    items: [
      {
        ...card,
        media: {
          ...card.media,
          title:
            query.health === "metadata_failed"
              ? "Failed media from server"
              : card.media.title,
        },
      },
    ],
    next_cursor: null,
  }));
  const { onSearch } = show(initial);
  await screen.findByRole("link", { name: "Details: Synthetic film" });
  const shortcuts = screen.getByRole("group", {
    name: "Filter by review status",
  });
  expect(
    within(shortcuts).getByRole("button", { name: "All" }),
  ).toHaveAttribute("aria-pressed", "true");
  const presets = [
    ["Missing details", "incomplete_metadata"],
    ["Needs review", "metadata_needs_review"],
    ["Source not configured", "metadata_unconfigured"],
    ["Metadata failed", "metadata_failed"],
    ["Missing poster", "missing_poster"],
  ] as const;
  for (const [label, health] of presets) {
    const button = within(shortcuts).getByRole("button", {
      name: label,
    });
    button.focus();
    fireEvent.click(button);
    expect(onSearch).toHaveBeenLastCalledWith(
      { ...initial, health, cursor: undefined, page: 1 },
      undefined,
    );
    await waitFor(() =>
      expect(libraryApi.media).toHaveBeenLastCalledWith(
        expect.objectContaining({
          health,
          cursor: null,
          root_id: root.id,
          kind: "movie",
          query: "Scoped search",
          sort: "year_desc",
          genre: "Drama",
          include_adult: false,
        }),
      ),
    );
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button).toHaveFocus();
    if (health === "metadata_failed") {
      expect(
        await screen.findByRole("link", {
          name: "Details: Failed media from server",
        }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("link", { name: "Details: Synthetic film" }),
      ).not.toBeInTheDocument();
    }
    const calls = onSearch.mock.calls.length;
    fireEvent.click(button);
    expect(onSearch).toHaveBeenCalledTimes(calls);
  }
  fireEvent.click(within(shortcuts).getByRole("button", { name: "All" }));
  expect(onSearch).toHaveBeenLastCalledWith(
    { ...initial, health: undefined, cursor: undefined, page: 1 },
    undefined,
  );
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        health: null,
        root_id: root.id,
        kind: "movie",
        query: "Scoped search",
        sort: "year_desc",
        genre: "Drama",
        cursor: null,
      }),
    ),
  );
  expect(adultApi.media).not.toHaveBeenCalled();
});

it("keeps the advanced health selector and shortcuts on one effective query", async () => {
  show({ health: "missing_nfo" });
  await screen.findByRole("link", { name: "Details: Synthetic film" });
  const shortcuts = screen.getByRole("group", {
    name: "Filter by review status",
  });
  expect(
    within(shortcuts).getByRole("button", { name: "All" }),
  ).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(screen.getByRole("button", { name: /^Filters/ }));
  const health = screen.getByRole("combobox", { name: "Health" });
  expect(health).toHaveValue("missing_nfo");
  chooseSelectOption(health, "missing_poster");
  expect(
    within(shortcuts).getByRole("button", {
      name: "Missing poster",
    }),
  ).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(
    within(shortcuts).getByRole("button", {
      name: "Needs review",
    }),
  );
  expect(health).toHaveValue("metadata_needs_review");
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        health: "metadata_needs_review",
        cursor: null,
      }),
    ),
  );
});

it("exposes only supported adult shortcuts and clears private health when returning to ordinary media", async () => {
  privacy.settings.entry_visible = true;
  privacy.includeAdult = true;
  show({ kind: "adult_video", root: root.id, cursor: "private-page", page: 3 });
  await screen.findByRole("link", { name: "Details: Synthetic film" });
  const shortcuts = screen.getByRole("group", {
    name: "Filter by review status",
  });
  expect(
    within(shortcuts)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["All", "Missing details", "Missing poster"]);
  expect(libraryApi.media).not.toHaveBeenCalled();
  expect(libraryApi.roots).not.toHaveBeenCalled();
  expect(libraryApi.facets).not.toHaveBeenCalled();
  fireEvent.click(
    within(shortcuts).getByRole("button", {
      name: "Missing poster",
    }),
  );
  await waitFor(() =>
    expect(adultApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "adult_video",
        root_id: root.id,
        health: "missing_poster",
        include_adult: true,
        cursor: null,
      }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: /^Filters/ }));
  const options = selectOptions(screen.getByRole("combobox", { name: "Health" }));
  expect(options.map((option) => option.textContent)).not.toContain("Source not configured");
  for (const [query] of vi.mocked(adultApi.media).mock.calls)
    expect(query.health?.startsWith("metadata_")).not.toBe(true);
  chooseSelectOption(screen.getByRole("combobox", { name: "Media type" }), "movie");
  await waitFor(() =>
    expect(libraryApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "movie",
        include_adult: false,
        root_id: null,
        health: null,
        cursor: null,
      }),
    ),
  );
});

it.each([
  "metadata_pending",
  "metadata_needs_review",
  "metadata_unconfigured",
  "metadata_failed",
])(
  "ignores unsupported adult metadata health from a manual URL: %s",
  async (health) => {
    privacy.settings.entry_visible = true;
    privacy.includeAdult = true;
    const { onSearch } = show({
      kind: "adult_video",
      root: root.id,
      health,
      cursor: "manual-private-page",
      page: 2,
    });
    await screen.findByRole("link", { name: "Details: Synthetic film" });
    expect(adultApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        include_adult: true,
        kind: "adult_video",
        health: null,
      }),
    );
    const shortcuts = screen.getByRole("group", {
      name: "Filter by review status",
    });
    const all = within(shortcuts).getByRole("button", { name: "All" });
    expect(all).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: /^Filters/ }));
    expect(screen.getByRole("combobox", { name: "Health" })).toHaveValue("");
    fireEvent.click(all);
    expect(onSearch).toHaveBeenLastCalledWith(
      {
        kind: "adult_video",
        root: root.id,
        health: undefined,
        cursor: undefined,
        page: 1,
      },
      undefined,
    );
    await waitFor(() =>
      expect(adultApi.media).toHaveBeenLastCalledWith(
        expect.objectContaining({ health: null, cursor: null }),
      ),
    );
    expect(libraryApi.media).not.toHaveBeenCalled();
    expect(libraryApi.facets).not.toHaveBeenCalled();
  },
);
