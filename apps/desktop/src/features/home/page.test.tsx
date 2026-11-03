import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import {
  defaultBrowse,
  libraryApi,
  type MediaCard,
  type MediaDetail,
  type ScanTask,
} from "../../api/library";
import i18n from "../../i18n";
import { DashboardPage } from "./page";

vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    children,
    ...props
  }: {
    to: string;
    children: React.ReactNode;
  }) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    media: vi.fn(),
    summary: vi.fn(),
    detail: vi.fn(),
    tasks: vi.fn(),
  },
}));
const media: MediaCard = {
  media: {
    id: "fixture-ordinary",
    title: "Synthetic ordinary title",
    kind: "movie",
    year: 2026,
    original_title: null,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  },
  poster_uri: "data:image/png;base64,ZmFrZQ==",
  health: [],
};
const detail: MediaDetail = {
  ...media,
  files: [],
  artworks: [],
  external_ids: [],
  relations: [],
  metadata_values: [
    {
      id: "plot",
      field_key: "plot",
      locale: null,
      value: "Synthetic verified synopsis",
      provider_id: null,
      source_url: null,
      is_user_modified: false,
      is_locked: false,
    },
  ],
};
function show() {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <DashboardPage />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.summary).mockResolvedValue({
    total_media: 1,
    total_roots: 1,
    pending_tasks: 0,
    failed_tasks: 0,
    missing_poster: 0,
    missing_nfo: 0,
    incomplete_metadata: 0,
  });
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [media],
    next_cursor: null,
  });
  vi.mocked(libraryApi.detail).mockResolvedValue(detail);
});
it("renders real ordinary media, its synopsis and system-open actions without invented watching progress", async () => {
  show();
  expect(
    await screen.findByRole("heading", { level: 1, name: media.media.title }),
  ).toBeInTheDocument();
  expect(
    await screen.findByText("Synthetic verified synopsis"),
  ).toBeInTheDocument();
  expect(libraryApi.media).toHaveBeenCalledWith({
    ...defaultBrowse(false),
    sort: "added_desc",
    limit: 14,
  });
  expect(libraryApi.summary).toHaveBeenCalledWith(defaultBrowse(false));
  expect(libraryApi.detail).toHaveBeenCalledWith(media.media.id, false);
  expect(
    screen.getAllByRole("button", {
      name: `Open in system app: ${media.media.title}`,
    }),
  ).toHaveLength(2);
  expect(
    screen.getByRole("heading", { name: "Recently added" }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  expect(screen.queryByText(/continue watching/i)).not.toBeInTheDocument();
});
it("does not promote adult media or request its detail if the ordinary channel returns it", async () => {
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [
      {
        ...media,
        media: {
          ...media.media,
          id: "private",
          title: "Synthetic private title",
          kind: "adult_video",
        },
      },
    ],
    next_cursor: null,
  });
  show();
  expect(
    await screen.findByRole("heading", {
      name: "Your media belongs here.",
    }),
  ).toBeInTheDocument();
  expect(screen.queryByText("Synthetic private title")).not.toBeInTheDocument();
  expect(libraryApi.detail).not.toHaveBeenCalled();
  expect(
    screen.getByRole("link", { name: "Manage media folders" }),
  ).toHaveAttribute("href", "/roots");
});
it("offers a clear empty entry and shows the library request error without sample posters", async () => {
  vi.mocked(libraryApi.media).mockRejectedValue(
    new DesktopError("request_failed"),
  );
  show();
  expect(await screen.findByRole("alert")).toBeInTheDocument();
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  fireEvent.click(screen.getByRole("button", { name: "Check again" }));
  expect(
    await screen.findByRole("link", { name: "Manage media folders" }),
  ).toBeInTheDocument();
});
it("keeps a loading state until media arrives", async () => {
  let resolve!: (value: { items: MediaCard[]; next_cursor: null }) => void;
  vi.mocked(libraryApi.media).mockReturnValue(
    new Promise((done) => {
      resolve = done;
    }),
  );
  show();
  expect(screen.getByRole("status")).toHaveTextContent("Loading");
  resolve({ items: [], next_cursor: null });
  await waitFor(() =>
    expect(screen.queryByRole("status")).not.toBeInTheDocument(),
  );
});

it("offers directory selection when no roots exist", async () => {
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  vi.mocked(libraryApi.summary).mockResolvedValue({
    total_media: 0,
    total_roots: 0,
    pending_tasks: 0,
    failed_tasks: 0,
    missing_poster: 0,
    missing_nfo: 0,
    incomplete_metadata: 0,
  });
  show();
  expect(await screen.findByRole("link", { name: "Add root" })).toHaveAttribute(
    "href",
    "/roots",
  );
  expect(
    screen.queryByRole("link", { name: "Start scan" }),
  ).not.toBeInTheDocument();
});
it("shows actual automatic processing without asking the user to import candidates", async () => {
  vi.mocked(libraryApi.media).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  vi.mocked(libraryApi.summary).mockResolvedValue({
    total_media: 0,
    total_roots: 1,
    pending_tasks: 1,
    failed_tasks: 0,
    missing_poster: 0,
    missing_nfo: 0,
    incomplete_metadata: 0,
  });
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    {
      id: "automatic-refresh",
      root_id: "ordinary-folder",
      state: "running",
      mode: "automatic",
      phase: "metadata",
      visited_files: 4,
      skipped_files: 0,
      candidate_count: 4,
      imported_count: 4,
      matched_count: 2,
      review_count: 1,
      metadata_failed_count: 0,
      warnings: 0,
      error_code: null,
      created_at: "2026-11-03T10:00:00Z",
      updated_at: "2026-11-03T10:00:01Z",
    } satisfies ScanTask,
  ]);
  show();
  expect(
    await screen.findByRole("link", { name: "View processing status" }),
  ).toHaveAttribute("href", "/scanner");
  expect(screen.getByText("Fetching media details")).toBeInTheDocument();
  expect(screen.getByText("4 items added")).toBeInTheDocument();
  expect(screen.getByText("2 items identified")).toBeInTheDocument();
  expect(screen.getByText("1 items to review")).toBeInTheDocument();
  expect(
    screen.queryByText(/confirm candidates|confirm.*import/i),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Add root" }),
  ).not.toBeInTheDocument();
});
