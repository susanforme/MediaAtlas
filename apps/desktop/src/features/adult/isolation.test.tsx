import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { defaultBrowse, libraryApi } from "../../api/library";
import { providerApi } from "../../api/providers";
import i18n from "../../i18n";
import { LibraryPage } from "../library/browse";
import { MediaDetailPage } from "../library/detail";
import { DashboardPage, RootsPage } from "../library/pages";
import { ScannerPage, TasksPage } from "../library/scanner";

vi.mock("../library/context", () => ({
  usePrivacy: () => ({
    includeAdult: true,
    settings: {
      entry_visible: true,
      covers_visible: true,
      remember_recent: true,
      unlocked: true,
    },
  }),
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    summary: vi.fn(),
    roots: vi.fn(),
    tasks: vi.fn(),
    media: vi.fn(),
    facets: vi.fn(),
    detail: vi.fn(),
  },
}));
vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { snapshot: vi.fn() },
}));
function show(node: React.ReactNode) {
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
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
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
  vi.mocked(libraryApi.summary).mockResolvedValue({
    total_media: 0,
    total_roots: 0,
    pending_tasks: 0,
    failed_tasks: 0,
    missing_poster: 0,
    missing_nfo: 0,
    incomplete_metadata: 0,
  });
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1,
    providers: [],
    tasks: [],
    requests: [],
  });
  vi.mocked(libraryApi.detail).mockRejectedValue(
    new Error("Synthetic ordinary channel not found"),
  );
});
it("keeps dashboard and ordinary library queries adult-free after unlock", async () => {
  show(
    <>
      <DashboardPage />
      <LibraryPage
        search={{ kind: "adult_video", q: "Synthetic" }}
        onSearch={() => {}}
      />
    </>,
  );
  await waitFor(() => expect(libraryApi.media).toHaveBeenCalled());
  expect(libraryApi.summary).toHaveBeenCalledWith(defaultBrowse(false));
  expect(libraryApi.media).toHaveBeenCalledWith(
    expect.objectContaining({ include_adult: false, kind: null }),
  );
  expect(libraryApi.facets).toHaveBeenCalledWith(
    expect.objectContaining({ include_adult: false, kind: null }),
  );
  expect(libraryApi.roots).toHaveBeenCalledWith(false);
});
it("keeps ordinary roots, scanner and task lists separate after unlock", async () => {
  show(
    <>
      <RootsPage />
      <ScannerPage search={{}} onSearch={() => {}} />
      <TasksPage />
    </>,
  );
  await waitFor(() => expect(libraryApi.tasks).toHaveBeenCalled());
  for (const [includeAdult] of vi.mocked(libraryApi.roots).mock.calls)
    expect(includeAdult).toBe(false);
  for (const [includeAdult] of vi.mocked(libraryApi.tasks).mock.calls)
    expect(includeAdult).toBe(false);
});
it("never uses a session unlock to expose adult media through ordinary detail routes", async () => {
  show(<MediaDetailPage mediaId="synthetic-adult-id" />);
  await waitFor(() =>
    expect(libraryApi.detail).toHaveBeenCalledWith("synthetic-adult-id", false),
  );
});
