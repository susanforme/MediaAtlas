import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { useState } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import {
  libraryApi,
  type MediaCard,
  type MediaKind,
  type RootView,
} from "../../api/library";
import { reviewQueueApi } from "../../api/review-queue";
import { Button } from "../../components/ui/button";
import i18n from "../../i18n";
import { chooseSelectOption, selectOptions } from "../../test/select";
import { MAX_SCRAPE_QUEUE } from "../scraper/search";
import { OrdinaryCandidates } from "./ordinary-candidates";
import { LocalCandidates } from "./page";
import { TOOLS_PAGE_SIZE, type ToolsRouteSearch } from "./search";
import "./translations";

const queueNavigate = vi.hoisted(() => vi.fn());
vi.mock("../../api/review-queue", () => ({
  reviewQueueApi: { list: vi.fn(), create: vi.fn() },
}));
vi.mock("@tanstack/react-router", async (original) => {
  const router = await original<typeof import("@tanstack/react-router")>();
  return {
    ...router,
    useNavigate: () => queueNavigate,
    Link: ({
      to,
      search,
      disabled,
      ...props
    }: {
      to: string;
      search?: Record<string, unknown>;
      disabled?: boolean;
      children: React.ReactNode;
    }) => (
      <a
        href={disabled ? undefined : `${to}${router.defaultStringifySearch(search ?? {})}`}
        role={disabled ? "link" : undefined}
        aria-disabled={disabled || undefined}
        {...props}
      />
    ),
  };
});
vi.mock("../../api/library", async (original) => {
  const module = await original<typeof import("../../api/library")>();
  return {
    ...module,
    libraryApi: { ...module.libraryApi, roots: vi.fn(), toolsMedia: vi.fn() },
  };
});
vi.mock("../../api/adult", async (original) => {
  const module = await original<typeof import("../../api/adult")>();
  return {
    ...module,
    adultApi: { ...module.adultApi, toolsMedia: vi.fn(), roots: vi.fn() },
  };
});
const privacy = vi.hoisted(() => ({ enabled: true, allowed: true }));
vi.mock("../library/context", () => ({
  usePrivacy: () => ({
    includeAdult: privacy.allowed,
    settings: { entry_visible: privacy.enabled },
    loading: false,
  }),
}));
const time = "2026-01-01";
const root: RootView = {
  id: "root-a",
  name: "Fixture directory",
  path: "/fixture",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
  created_at: time,
  updated_at: time,
  last_scan_at: null,
};
function card(
  id: string,
  kind: MediaKind = "movie",
  title = `Fixture ${id}`,
): MediaCard {
  return {
    media: {
      id,
      kind,
      title,
      original_title: null,
      year: 2020,
      created_at: time,
      updated_at: time,
    },
    poster_uri: null,
    health: ["missing_nfo"],
  };
}
const first = card("first");
const second = card("second", "tv_series");
const third = card("third", "anime");
const episode = card("episode", "tv_episode");
const privateCard = card("private", "adult_video", "Synthetic private title");
function Controlled({
  initialSearch = {},
}: {
  initialSearch?: ToolsRouteSearch;
}) {
  const [search, setSearch] = useState(initialSearch);
  return (
    <OrdinaryCandidates
      search={search}
      onSearch={setSearch}
      renderDetail={(id, close) => (
        <section>
          <h2>Detailed resource: {id}</h2>
          <Button onClick={close}>Close fixture detail</Button>
        </section>
      )}
    />
  );
}
function show(initialSearch: ToolsRouteSearch = {}) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <Controlled initialSearch={initialSearch} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
async function selectedQueue(): Promise<string[]> {
  const calls = vi.mocked(reviewQueueApi.create).mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "Scrape selected" }));
  await waitFor(() =>
    expect(reviewQueueApi.create).toHaveBeenCalledTimes(calls + 1),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Scrape selected" }),
    ).toBeEnabled(),
  );
  return vi.mocked(reviewQueueApi.create).mock.calls.at(-1)?.[0] ?? [];
}
beforeEach(async () => {
  vi.resetAllMocks();
  vi.mocked(reviewQueueApi.list).mockResolvedValue([]);
  vi.mocked(reviewQueueApi.create).mockImplementation(async (ids, context) => ({
    id: "11111111-1111-4111-8111-111111111111",
    revision: 0,
    current_media_id: ids[0],
    context,
    items: ids.map((media_id, position) => ({
      media_id,
      position,
      state: "pending",
      checkpoint: null,
      preview_id: null,
      needs_recheck: false,
      reason: null,
    })),
    created_at: time,
    updated_at: time,
  }));
  privacy.enabled = true;
  privacy.allowed = true;
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([
    root,
    {
      ...root,
      id: "private-root",
      name: "Synthetic private root",
      kind: "adult_video",
    },
  ]);
  vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
    items: [first, second, third, episode, privateCard, first],
    next_cursor: null,
  });
});

it("uses the same type selector and table while switching backend adapters and clearing ordinary selection", async () => {
  vi.mocked(adultApi.toolsMedia).mockResolvedValue({
    items: [privateCard, first],
    next_cursor: null,
  });
  show();
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  );
  chooseSelectOption(screen.getByRole("combobox", { name: "Media type" }), "adult_video");
  expect(
    await screen.findByText(privateCard.media.title),
  ).toBeInTheDocument();
  expect(
    screen.queryByText(first.media.title),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
  expect(adultApi.toolsMedia).toHaveBeenCalledWith(
    expect.objectContaining({
      kind: "adult_video",
      include_adult: true,
      limit: TOOLS_PAGE_SIZE,
    }),
  );
  chooseSelectOption(screen.getByRole("combobox", { name: "Media type" }), "movie");
  expect(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  ).not.toBeChecked();
  expect(
    screen.queryByText(privateCard.media.title),
  ).not.toBeInTheDocument();
});

it("does not fetch or render the adult type while its setting is disabled", async () => {
  privacy.enabled = false;
  privacy.allowed = false;
  show({ kind: "adult_video" });
  selectOptions(screen.getByRole("combobox", { name: "Media type" }));
  await waitFor(() =>
    expect(screen.getByRole("option", { name: "Adult video" })).toBeDisabled(),
  );
  expect(adultApi.toolsMedia).not.toHaveBeenCalled();
  expect(adultApi.roots).not.toHaveBeenCalled();
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
});

it("renders ordinary candidate metadata in a table with independent view and selection actions", async () => {
  show();
  expect(await screen.findByRole("table")).toBeInTheDocument();
  expect(screen.getAllByText(first.media.title)).toHaveLength(1);
  expect(screen.getAllByText(/^2020 · /)).toHaveLength(4);
  expect(screen.getByText("2020 · Movie")).toBeInTheDocument();
  expect(screen.queryByText("Synthetic private title")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "Library root" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Filters" }));
  selectOptions(screen.getByRole("combobox", { name: "Library root" }));
  expect(
    screen.queryByRole("option", { name: "Synthetic private root" }),
  ).not.toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole("combobox", { name: "Library root" }), { key: "Escape" });
  expect(
    screen.queryByLabelText("Choose a local resource"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${episode.media.title}`,
    }),
  ).toBeDisabled();
  fireEvent.click(
    screen.getByRole("button", { name: `View resource: ${first.media.title}` }),
  );
  expect(
    screen.getByRole("heading", { name: "Detailed resource: first" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Resource tools" })).toHaveFocus();
  expect(
    screen.queryByRole("button", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${second.media.title}`,
    }),
  );
  expect(await selectedQueue()).toEqual([second.media.id]);
  fireEvent.click(screen.getByRole("button", { name: "Close fixture detail" }));
  expect(
    screen.queryByRole("heading", { name: "Detailed resource: first" }),
  ).not.toBeInTheDocument();
  expect(await selectedQueue()).toEqual([second.media.id]);
});

it("sends every filter and sort to the backend instead of filtering only the visible page", async () => {
  vi.mocked(libraryApi.toolsMedia).mockImplementation(async (query) => ({
    items:
      query.query === "Server result"
        ? [card("server", "movie", "Server result")]
        : [first],
    next_cursor: null,
  }));
  show({
    root: root.id,
    kind: "movie",
    health: "missing_nfo",
    sort: "title_asc",
    cursor: "deep-page",
    page: 4,
  });
  await screen.findByText(first.media.title);
  expect(libraryApi.toolsMedia).toHaveBeenLastCalledWith(
    expect.objectContaining({
      root_id: root.id,
      kind: "movie",
      health: "missing_nfo",
      sort: "title_asc",
      cursor: "deep-page",
      include_adult: false,
      limit: TOOLS_PAGE_SIZE,
    }),
  );
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  );
  fireEvent.change(screen.getByRole("searchbox", { name: "Search local candidates" }), {
    target: { value: "Server result" },
  });
  expect(screen.getByRole("checkbox", {
    name: `Select for scraping: ${first.media.title}`,
  })).toBeChecked();
  expect(screen.queryByText("Server result")).not.toBeInTheDocument();
  expect(libraryApi.toolsMedia).toHaveBeenCalledTimes(1);
  fireEvent.submit(screen.getByRole("search", { name: "Search local candidates" }));
  expect(await screen.findByText("Server result")).toBeInTheDocument();
  expect(libraryApi.toolsMedia).toHaveBeenLastCalledWith(
    expect.objectContaining({
      query: "Server result",
      root_id: root.id,
      kind: "movie",
      health: "missing_nfo",
      sort: "title_asc",
      cursor: null,
      include_adult: false,
    }),
  );
  expect(screen.getByText("Page 1")).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
  const serverSelection = screen.getByRole("checkbox", {
    name: "Select for scraping: Server result",
  });
  fireEvent.click(serverSelection);
  const queries = vi.mocked(libraryApi.toolsMedia).mock.calls.length;
  fireEvent.submit(screen.getByRole("search", { name: "Search local candidates" }));
  expect(serverSelection).toBeChecked();
  expect(libraryApi.toolsMedia).toHaveBeenCalledTimes(queries);
});

it("retains selection across cursor pages and creates a durable queue from ordered, deduplicated backend IDs", async () => {
  vi.mocked(libraryApi.toolsMedia).mockImplementation(async (query) =>
    query.cursor
      ? { items: [third], next_cursor: null }
      : { items: [first, second, first], next_cursor: "page-two" },
  );
  show();
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${second.media.title}`,
    }),
  );
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "More candidates" }));
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${third.media.title}`,
    }),
  );
  expect(await selectedQueue()).toEqual([
    second.media.id,
    first.media.id,
    third.media.id,
  ]);
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  );
  expect(await selectedQueue()).toEqual([second.media.id, first.media.id]);
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  );
  expect(await selectedQueue()).toEqual([
    second.media.id,
    first.media.id,
    third.media.id,
  ]);

  expect(queueNavigate).toHaveBeenLastCalledWith({
    to: "/scraper",
    search: {
      session: "11111111-1111-4111-8111-111111111111",
      media: second.media.id,
    },
  });
  expect(vi.mocked(reviewQueueApi.create).mock.calls.at(-1)?.[1]).toMatchObject(
    { cursor: "page-two", page: 2 },
  );
  fireEvent.click(screen.getByRole("button", { name: "Previous candidates" }));
  expect(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  ).toBeChecked();
  expect(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${second.media.title}`,
    }),
  ).toBeChecked();
  expect(await selectedQueue()).toHaveLength(3);
});

it("selects only supported ordinary media on the current page and can remove an off-page selection", async () => {
  show();
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  );
  expect(await selectedQueue()).toEqual([
    first.media.id,
    second.media.id,
    third.media.id,
  ]);
  expect(screen.getByRole("region", { name: "3 selected" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: `View resource: ${first.media.title}` })).toBeDisabled();
  expect(screen.getAllByRole("link", { name: "Scrape this media" })[0]).toHaveAttribute("aria-disabled", "true");
  expect(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${episode.media.title}`,
    }),
  ).not.toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "3 selected" }));
  fireEvent.click(
    screen.getByRole("button", {
      name: `Remove from scrape selection: ${second.media.title}`,
    }),
  );
  expect(await selectedQueue()).toEqual([first.media.id, third.media.id]);
  expect(
    screen.getByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  ).toHaveProperty("indeterminate", true);
  fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
  expect(
    screen.queryByRole("link", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
});

it("rejects a whole-page addition above the limit without silently truncating the queue", async () => {
  const pageOne = Array.from({ length: 24 }, (_, index) =>
    card(`one-${index}`),
  );
  const pageTwo = Array.from({ length: 12 }, (_, index) =>
    card(`two-${index}`),
  );
  vi.mocked(libraryApi.toolsMedia).mockImplementation(async (query) =>
    query.cursor
      ? { items: pageTwo, next_cursor: null }
      : { items: pageOne, next_cursor: "page-two" },
  );
  show();
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "More candidates" }));
  await screen.findByText(pageTwo[0].media.title);
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Select supported media on this page",
    }),
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    `exceed ${MAX_SCRAPE_QUEUE} items`,
  );
  expect(await selectedQueue()).toEqual(pageOne.map((item) => item.media.id));
  for (const item of pageTwo.slice(0, 8))
    fireEvent.click(
      screen.getByLabelText(`Select for scraping: ${item.media.title}`),
    );
  expect(await selectedQueue()).toHaveLength(MAX_SCRAPE_QUEUE);
  fireEvent.click(
    screen.getByLabelText(`Select for scraping: ${pageTwo[8].media.title}`),
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Nothing was added");
  expect(await selectedQueue()).toEqual(
    [...pageOne, ...pageTwo.slice(0, 8)].map((item) => item.media.id),
  );
}, 15000);

it.each([
  ["Library root", root.id, "root_id"],
  ["Media type", "anime", "kind"],
  ["Health", "missing_poster", "health"],
  ["Sort", "added_desc", "sort"],
])(
  "clears stale page and batch selection when the %s filter changes",
  async (label, value, field) => {
    vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
      items: [first],
      next_cursor: "page-two",
    });
    show({ cursor: "old-page", page: 3 });
    fireEvent.click(
      await screen.findByRole("checkbox", {
        name: `Select for scraping: ${first.media.title}`,
      }),
    );
    if (label === "Library root" || label === "Health")
      fireEvent.click(screen.getByRole("button", { name: /^Filters/ }));
    chooseSelectOption(screen.getByLabelText(label), value);
    await waitFor(() =>
      expect(libraryApi.toolsMedia).toHaveBeenLastCalledWith(
        expect.objectContaining({ [field]: value, cursor: null }),
      ),
    );
    expect(screen.getByText("Page 1")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Scrape selected" }),
    ).not.toBeInTheDocument();
  },
);

it("removes a disclosed filter without losing type or the other server filters", async () => {
  show({ kind: "movie", root: root.id, health: "missing_nfo", q: "Applied query", sort: "title_asc" });
  await screen.findByRole("table");
  const remove = screen.getByRole("button", { name: "Remove filter: Health · NFO not generated" });
  remove.focus();
  fireEvent.click(remove);
  expect(screen.getByRole("button", { name: /^Filters/ })).toHaveFocus();
  await waitFor(() => expect(libraryApi.toolsMedia).toHaveBeenLastCalledWith(
    expect.objectContaining({ kind: "movie", root_id: root.id, health: null, query: "Applied query", sort: "title_asc", cursor: null }),
  ));
  expect(screen.queryByRole("button", { name: "Remove filter: Health · NFO not generated" })).not.toBeInTheDocument();
});

it("keeps the adult resource type when clearing filters", async () => {
  vi.mocked(adultApi.toolsMedia).mockResolvedValue({ items: [privateCard], next_cursor: null });
  show({ kind: "adult_video", q: "Old query", health: "missing_nfo" });
  await screen.findByRole("table");
  fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
  await waitFor(() => expect(adultApi.toolsMedia).toHaveBeenLastCalledWith(
    expect.objectContaining({ kind: "adult_video", include_adult: true, query: null, health: null }),
  ));
  expect(libraryApi.toolsMedia).not.toHaveBeenCalled();
  expect(screen.getByRole("searchbox")).toHaveValue("");
});

it("does not reveal an adult directory name in an ordinary active-filter summary", async () => {
  show({ kind: "movie", root: "private-root" });
  await screen.findByRole("table");
  expect(screen.queryByText(/Synthetic private root/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /^Filters/ }));
  const options = selectOptions(screen.getByRole("combobox", { name: "Library root" }));
  expect(options.some((option) => option.textContent?.includes("Synthetic private root"))).toBe(false);
});

it("keeps adult candidates on their independent endpoint without ordinary multi-select or queue links", async () => {
  vi.mocked(adultApi.toolsMedia).mockResolvedValue({
    items: [privateCard],
    next_cursor: null,
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <LocalCandidates adult />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  expect(
    await screen.findByText(privateCard.media.title),
  ).toBeInTheDocument();
  expect(adultApi.toolsMedia).toHaveBeenCalledWith(
    expect.objectContaining({ include_adult: true }),
  );
  expect(libraryApi.toolsMedia).not.toHaveBeenCalled();
  expect(libraryApi.roots).not.toHaveBeenCalled();
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
});

it("provides retry feedback for backend candidate errors without adding fabricated results", async () => {
  vi.mocked(libraryApi.toolsMedia).mockRejectedValueOnce(
    new DesktopError("request_failed"),
  );
  show();
  const error = await screen.findByRole("alert");
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  fireEvent.click(within(error).getByRole("button", { name: "Check again" }));
  expect(await screen.findByRole("table")).toBeInTheDocument();
});

it("does not overwrite a pending router reset with the previous filter state", async () => {
  const onSearch = vi.fn();
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <OrdinaryCandidates
          search={{ q: "Old query", cursor: "old-page", page: 3 }}
          onSearch={onSearch}
          renderDetail={() => null}
        />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
  expect(
    screen.queryByRole("button", { name: "Scrape selected" }),
  ).not.toBeInTheDocument();
  expect(onSearch).toHaveBeenCalledExactlyOnceWith({}, false);
});

it("offers an explicit saved-session resume without recreating or replaying the queue", async () => {
  vi.mocked(reviewQueueApi.list).mockResolvedValue([
    {
      id: "11111111-1111-4111-8111-111111111111",
      revision: 4,
      current_media_id: second.media.id,
      context: { q: "saved filter", kind: "movie" },
      items: [first, second].map((card, position) => ({
        media_id: card.media.id,
        position,
        state: position ? "pending" : "deferred",
        checkpoint: null,
        preview_id: null,
        needs_recheck: false,
        reason: null,
      })),
      created_at: time,
      updated_at: time,
    },
  ]);
  show();
  const resume = await screen.findByRole("link", { name: "Resume review" });
  const url = new URL(
    resume.getAttribute("href") ?? "",
    "https://fixture.invalid",
  );
  expect(url.pathname).toBe("/scraper");
  expect(url.searchParams.get("session")).toBe(
    "11111111-1111-4111-8111-111111111111",
  );
  expect(url.searchParams.get("media")).toBe(second.media.id);
  expect(reviewQueueApi.create).not.toHaveBeenCalled();
  expect(queueNavigate).not.toHaveBeenCalled();
});

it("keeps selection available after a failed queue creation instead of navigating to a fictitious session", async () => {
  vi.mocked(reviewQueueApi.create).mockRejectedValueOnce(
    new DesktopError("request_failed"),
  );
  show();
  fireEvent.click(
    await screen.findByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Scrape selected" }));
  await screen.findByRole("alert");
  expect(queueNavigate).not.toHaveBeenCalled();
  expect(
    screen.getByRole("checkbox", {
      name: `Select for scraping: ${first.media.title}`,
    }),
  ).toBeChecked();
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Scrape selected" }),
    ).toBeEnabled(),
  );
  expect(await selectedQueue()).toEqual([first.media.id]);
});
