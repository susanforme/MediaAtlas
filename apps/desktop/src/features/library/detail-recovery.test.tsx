import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import {
  libraryApi,
  type MediaDetail,
  type MediaKind,
} from "../../api/library";
import i18n from "../../i18n";
import { MediaDetailPage } from "./detail";
import { detailRecovery } from "./detail-recovery-model";

vi.mock("./context", () => ({
  usePrivacy: () => ({
    includeAdult: true,
    settings: { entry_visible: true, covers_visible: true },
  }),
}));
vi.mock("./use-library-activity", () => ({ useLibraryActivity: vi.fn() }));
vi.mock("../../api/library", () => ({ libraryApi: { detail: vi.fn() } }));
vi.mock("../../api/adult", () => ({ adultApi: { getMedia: vi.fn() } }));
vi.mock("../../api/scraper", () => ({
  scraperApi: { mediaDetail: vi.fn().mockResolvedValue(null) },
}));
vi.mock("../scraper/source-detail", () => ({
  LibraryVideoSourceDetail: () => null,
  VideoSourceDetails: () => null,
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    children,
  }: {
    to: string;
    search?: Record<string, string>;
    children: ReactNode;
  }) => (
    <a href={`${to}?${new URLSearchParams(search).toString()}`}>{children}</a>
  ),
}));

function fixture(
  health: string[],
  roots = ["actual-root"],
  kind: MediaKind = "movie",
): MediaDetail {
  return {
    media: {
      id: "local-item",
      kind,
      title: "Local film",
      original_title: null,
      year: null,
      created_at: "2026-11-04T10:00:00Z",
      updated_at: "2026-11-04T10:00:00Z",
    },
    health,
    files: roots.map((root, index) => ({
      id: `file-${index}`,
      media_item_id: "local-item",
      root_id: root,
      relative_path: "Local.mkv",
      size_bytes: 1024,
      modified_at: null,
      part: null,
    })),
    poster_uri: null,
    artworks: [],
    external_ids: [],
    metadata_values: [],
    relations: [],
  };
}

async function show(detail: MediaDetail) {
  const adult = detail.media.kind === "adult_video";
  vi.mocked(libraryApi.detail).mockResolvedValue(detail);
  vi.mocked(adultApi.getMedia).mockResolvedValue(detail);
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <MediaDetailPage mediaId={detail.media.id} adult={adult} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  await screen.findByRole("heading", { level: 1, name: detail.media.title });
}

function setupSearch(label: string) {
  const link = screen.getByRole("link", { name: label });
  const url = new URL(
    link.getAttribute("href") ?? "",
    "https://mediaatlas.test",
  );
  expect(url.pathname).toBe("/settings");
  return Object.fromEntries(url.searchParams);
}

beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
});

it("offers source setup for the unique file root instead of an unusable identification action", async () => {
  await show(
    fixture(
      ["incomplete_metadata", "metadata_unconfigured"],
      ["actual-root", "actual-root"],
    ),
  );
  expect(setupSearch("Configure source and continue")).toEqual({
    from: "library",
    root: "actual-root",
    kind: "movie",
    provider: "tmdb",
    tab: "auth",
  });
  expect(
    screen.queryByRole("link", { name: /Identify media|Correct match/ }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open in system app" }),
  ).toBeEnabled();
  expect(screen.getByText("Source not configured")).toBeInTheDocument();
});

it.each([[], ["first-root", "second-root"]])(
  "does not promise to resume a guessed folder for files in %j",
  async (...roots) => {
    await show(fixture(["metadata_unconfigured"], roots));
    expect(setupSearch("Configure source")).not.toHaveProperty("root");
    expect(
      screen.queryByRole("link", { name: "Configure source and continue" }),
    ).not.toBeInTheDocument();
  },
);

it.each([
  ["tv_episode", "tv_series", "tmdb"],
  ["tv_season", "tv_series", "tmdb"],
  ["anime", "anime", "anilist"],
  ["adult_video", "adult_video", "dmm"],
] as const)(
  "preserves the source category for %s",
  async (kind, setupKind, provider) => {
    await show(fixture(["metadata_unconfigured"], ["typed-root"], kind));
    expect(setupSearch("Configure source and continue")).toMatchObject({
      root: "typed-root",
      kind: setupKind,
      provider,
    });
    if (kind === "adult_video") {
      expect(adultApi.getMedia).toHaveBeenCalledWith("local-item");
      expect(libraryApi.detail).not.toHaveBeenCalled();
    } else {
      expect(libraryApi.detail).toHaveBeenCalledWith("local-item", false);
      expect(adultApi.getMedia).not.toHaveBeenCalled();
    }
  },
);

it("announces current processing once and does not start a competing identification flow", async () => {
  await show(fixture(["incomplete_metadata", "metadata_pending"]));
  expect(screen.getByRole("status")).toHaveTextContent("Identifying");
  expect(screen.getAllByText("Identifying")).toHaveLength(1);
  expect(
    screen.queryByRole("link", {
      name: /Identify media|Correct match|Configure source/,
    }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open in system app" }),
  ).toBeEnabled();
});

it.each(["metadata_needs_review", "metadata_failed"])(
  "keeps the existing exact-media correction route for %s",
  async (health) => {
    await show(fixture([health, "incomplete_metadata"]));
    expect(
      screen.getByRole("link", { name: "Identify media" }),
    ).toHaveAttribute("href", "/scraper?media=local-item");
    expect(
      screen.queryByRole("link", { name: /Configure source/ }),
    ).not.toBeInTheDocument();
  },
);

it("shows the actual child status without sending unsupported episode IDs to the scraper", async () => {
  await show(fixture(["metadata_needs_review"], ["actual-root"], "tv_episode"));
  expect(screen.getByText("Match needs review")).toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: /Identify media|Correct match/ }),
  ).not.toBeInTheDocument();
});

it("validates file root IDs before allowing a resume promise", () => {
  expect(
    detailRecovery(fixture(["metadata_unconfigured"], ["/unsafe/root"])),
  ).toEqual({
    action: "configure",
    search: { from: "library", kind: "movie", provider: "tmdb", tab: "auth" },
  });
});

it.each(["music_release", "music_track", "music_video", "concert"] as const)(
  "does not promise unsupported automatic online matching for %s",
  (kind) => {
    expect(
      detailRecovery(fixture(["metadata_unconfigured"], ["actual-root"], kind)),
    ).toEqual({ action: "none" });
  },
);

it("keeps missing NFO output separate from an identified media result", async () => {
  await show(fixture(["missing_nfo"]));
  const section = screen.getByRole("region", { name: "NFO file" });
  expect(section).toHaveTextContent("NFO not generated");
  expect(section).toHaveTextContent("preserves existing NFO files");
  expect(screen.getAllByText("NFO not generated")).toHaveLength(1);
  expect(
    screen.queryByRole("link", { name: "Identify media" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Manage media folders" }),
  ).toHaveAttribute("href", "/roots?");
});

it("reports automatic NFO failure independently and keeps private recovery scoped", async () => {
  await show(
    fixture(
      ["missing_nfo", "nfo_output_failed"],
      ["actual-root"],
      "adult_video",
    ),
  );
  const section = screen.getByRole("region", { name: "NFO file" });
  expect(section).toHaveTextContent("Automatic NFO output failed");
  expect(screen.getAllByText("Automatic NFO output failed")).toHaveLength(1);
  expect(screen.queryByText("NFO not generated")).not.toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Manage media folders" }),
  ).toHaveAttribute("href", "/roots?kind=adult_video");
  expect(libraryApi.detail).not.toHaveBeenCalled();
});
