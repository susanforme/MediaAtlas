import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import {
  libraryApi,
  type MediaCard,
  type MediaDetail,
} from "../../api/library";
import i18n from "../../i18n";
import { LibraryPage } from "./browse";
import { MediaDetailPage } from "./detail";

const privacy = vi.hoisted(() => ({
  includeAdult: true,
  settings: {
    entry_visible: true,
    covers_visible: true,
    remember_recent: false,
    unlocked: true,
  },
}));
vi.mock("./context", () => ({ usePrivacy: () => privacy }));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    media: vi.fn(),
    roots: vi.fn(),
    tasks: vi.fn(),
    facets: vi.fn(),
    detail: vi.fn(),
  },
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { media: vi.fn(), roots: vi.fn(), getMedia: vi.fn() },
}));
vi.mock("../scraper/source-detail", () => ({
  LibraryVideoSourceDetail: () => null,
  VideoSourceDetails: () => null,
}));

const card: MediaCard = {
  media: {
    id: "fixture-item",
    kind: "adult_video",
    title: "Synthetic category item",
    original_title: null,
    year: 2026,
    created_at: "2026-11-03T10:00:00Z",
    updated_at: "2026-11-03T10:00:00Z",
  },
  poster_uri: null,
  health: [],
};
const detail: MediaDetail = {
  ...card,
  files: [],
  artworks: [],
  external_ids: [],
  metadata_values: [],
  relations: [],
};

function show(node: ReactNode) {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        {node}
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

beforeEach(async () => {
  vi.clearAllMocks();
  privacy.includeAdult = true;
  privacy.settings.entry_visible = true;
  privacy.settings.covers_visible = true;
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  vi.mocked(libraryApi.facets).mockResolvedValue({
    genres: [],
    people: [],
    studios: [],
    years: [],
  });
  vi.mocked(adultApi.media).mockResolvedValue({
    items: [card],
    next_cursor: null,
  });
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  vi.mocked(adultApi.getMedia).mockResolvedValue(detail);
});

it("uses the same library and five category choices without lock or unlock actions", async () => {
  show(
    <LibraryPage adult search={{ kind: "adult_video" }} onSearch={vi.fn()} />,
  );
  await screen.findByText(card.media.title);
  expect(
    screen.getByRole("heading", { level: 1, name: "Media library" }),
  ).toBeInTheDocument();
  const options = within(
    screen.getByRole("combobox", { name: "Media type" }),
  ).getAllByRole("option");
  expect(options.map((option) => (option as HTMLOptionElement).value)).toEqual([
    "",
    "movie",
    "tv_series",
    "anime",
    "music_release",
    "adult_video",
  ]);
  expect(
    screen.queryByRole("button", { name: /lock|unlock/i }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText(/workspace|locked|unlock for/i),
  ).not.toBeInTheDocument();
  expect(adultApi.media).toHaveBeenCalled();
  expect(libraryApi.media).not.toHaveBeenCalled();
  expect(libraryApi.facets).not.toHaveBeenCalled();
});

it("removes disabled categories from the ordinary selector", async () => {
  privacy.settings.entry_visible = false;
  privacy.includeAdult = false;
  show(<LibraryPage search={{}} onSearch={vi.fn()} />);
  await screen.findByText("Your library is empty");
  expect(
    within(screen.getByRole("combobox", { name: "Media type" })).queryByRole(
      "option",
      { name: "Adult video" },
    ),
  ).not.toBeInTheDocument();
  expect(adultApi.media).not.toHaveBeenCalled();
});

it("uses the shared detail layout and actions with only a category-specific data adapter", async () => {
  show(
    <MediaDetailPage
      mediaId={card.media.id}
      adult
      sourceDetail={<p>Synthetic source fields</p>}
    />,
  );
  expect(
    await screen.findByRole("heading", { level: 1, name: card.media.title }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open in system app" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Correct match" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Files" })).toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Metadata and sources" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Synthetic source fields")).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /lock|unlock/i }),
  ).not.toBeInTheDocument();
  expect(adultApi.getMedia).toHaveBeenCalledWith(card.media.id);
  expect(libraryApi.detail).not.toHaveBeenCalled();
});

it("waits for the automatic backend grant without showing an unlock workflow", () => {
  privacy.includeAdult = false;
  show(<MediaDetailPage mediaId={card.media.id} adult />);
  expect(screen.getByRole("status")).toHaveTextContent("Loading");
  expect(adultApi.getMedia).not.toHaveBeenCalled();
  expect(
    screen.queryByRole("button", { name: /lock|unlock/i }),
  ).not.toBeInTheDocument();
});
