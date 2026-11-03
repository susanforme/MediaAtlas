import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { type AdultSettings, adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { providerApi } from "../../api/providers";
import i18n from "../../i18n";
import { fixtureSnapshot } from "../providers/fixtures";
import {
  syntheticAdultDetail,
  syntheticAdultMedia,
  syntheticAdultMerge,
  syntheticAdultOutput,
  syntheticAdultSearch,
} from "./fixtures";
import { AdultMediaPage, AdultPage, AdultToolsPage } from "./page";

const privacy = vi.hoisted(() => ({
  settings: {
    entry_visible: true,
    covers_visible: false,
    remember_recent: false,
    unlocked: true,
  } as AdultSettings,
  includeAdult: true,
  loading: false,
  error: undefined,
  unlock: vi.fn(),
  lock: vi.fn(),
}));
vi.mock("../library/context", () => ({ usePrivacy: () => privacy }));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: {
    media: vi.fn(),
    toolsMedia: vi.fn(),
    toolsDetail: vi.fn(),
    getMedia: vi.fn(),
    providers: vi.fn(),
    search: vi.fn(),
    detail: vi.fn(),
    preview: vi.fn(),
    confirm: vi.fn(),
    mediaDetail: vi.fn(),
    artworkPreview: vi.fn(),
    outputPreview: vi.fn(),
    outputExecute: vi.fn(),
    outputDiscard: vi.fn(),
    batchPreview: vi.fn(),
    batchExecute: vi.fn(),
    batchCancel: vi.fn(),
    roots: vi.fn(),
    tasks: vi.fn(),
    recent: vi.fn(),
  },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: {
    media: vi.fn(),
    roots: vi.fn(),
    tasks: vi.fn(),
    detail: vi.fn(),
  },
}));
vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { setEnabled: vi.fn(), cancel: vi.fn() },
}));
function show(node = <AdultPage />) {
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
  privacy.settings = {
    entry_visible: true,
    covers_visible: false,
    remember_recent: false,
    unlocked: true,
  };
  privacy.includeAdult = true;
  vi.mocked(adultApi.media).mockResolvedValue(syntheticAdultMedia);
  vi.mocked(adultApi.toolsMedia).mockResolvedValue(syntheticAdultMedia);
  vi.mocked(adultApi.recent).mockResolvedValue([]);
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  vi.mocked(adultApi.tasks).mockResolvedValue([]);
  const providers = fixtureSnapshot();
  providers.providers[0].descriptor = {
    ...providers.providers[0].descriptor,
    id: "dmm",
    name: "DMM / FANZA",
    fixture: false,
    media_kinds: ["adult_video"],
  };
  providers.providers[0].config.enabled = true;
  vi.mocked(adultApi.providers).mockResolvedValue(providers);
  vi.mocked(adultApi.search).mockResolvedValue(syntheticAdultSearch);
  vi.mocked(adultApi.detail).mockResolvedValue(syntheticAdultDetail);
  vi.mocked(adultApi.preview).mockResolvedValue(syntheticAdultMerge);
  vi.mocked(adultApi.confirm).mockResolvedValue({
    schema_version: 1,
    preview_id: "synthetic-merge",
    applied_fields: 1,
    already_applied: false,
  });
  vi.mocked(adultApi.artworkPreview).mockResolvedValue({
    thumbnail: "data:image/png;base64,aGVsbG8=",
  });
  vi.mocked(adultApi.outputPreview).mockResolvedValue(syntheticAdultOutput);
  vi.mocked(adultApi.outputDiscard).mockResolvedValue(null);
  vi.mocked(adultApi.outputExecute).mockResolvedValue({
    schema_version: 1,
    preview_id: "synthetic-output",
    written: 1,
    preserved: 0,
    already_applied: false,
    state: "succeeded",
  });
  vi.mocked(adultApi.batchPreview).mockResolvedValue({
    schema_version: 1,
    batch_id: "synthetic-batch",
    state: "planned",
    items: [syntheticAdultOutput],
  });
  const result = {
    schema_version: 1 as const,
    batch_id: "synthetic-batch",
    state: "cancelled",
    items: [
      {
        media_item_id: "synthetic-media",
        preview_id: "synthetic-output",
        state: "cancelled",
        result: null,
        error_code: null,
      },
    ],
  };
  vi.mocked(adultApi.batchCancel).mockResolvedValue(result);
  vi.mocked(adultApi.batchExecute).mockResolvedValue({
    ...result,
    state: "succeeded",
  });
});
async function candidate() {
  show(<AdultToolsPage />);
  fireEvent.click(screen.getByRole("button", { name: "Scraper" }));
  await screen.findByText("DMM / FANZA");
  fireEvent.click(screen.getByRole("checkbox", { name: "DMM / FANZA" }));
  fireEvent.change(screen.getByLabelText("Catalog number"), {
    target: { value: "SYNTHETIC-001" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Search catalog providers" }),
  );
  await screen.findByText("Catalog number matches");
  fireEvent.click(screen.getByRole("button", { name: "Review candidate" }));
  await screen.findByText("Synthetic Label");
}
it("keeps direct adult routes closed until explicitly unlocked and does not query private data", async () => {
  privacy.includeAdult = false;
  privacy.settings.unlocked = false;
  show();
  await screen.findByText("Adult workspace locked");
  expect(adultApi.media).not.toHaveBeenCalled();
  expect(adultApi.providers).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Unlock for this session" }),
  );
  expect(privacy.unlock).toHaveBeenCalledOnce();
});
it("hides a disabled adult route and never fetches private media details", () => {
  privacy.includeAdult = false;
  privacy.settings.entry_visible = false;
  show(<AdultMediaPage mediaId="synthetic-media" />);
  expect(
    screen.queryByText("Synthetic local adult fixture"),
  ).not.toBeInTheDocument();
  expect(adultApi.getMedia).not.toHaveBeenCalled();
  expect(adultApi.mediaDetail).not.toHaveBeenCalled();
});
it("keeps local adult searches and images separate from ordinary APIs", async () => {
  show();
  await screen.findByText("Synthetic local adult fixture");
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  expect(adultApi.recent).not.toHaveBeenCalled();
  expect(libraryApi.media).not.toHaveBeenCalled();
  expect(libraryApi.tasks).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Search adult library"), {
    target: { value: "SYNTHETIC" },
  });
  await waitFor(() =>
    expect(adultApi.media).toHaveBeenLastCalledWith(
      expect.objectContaining({
        query: "SYNTHETIC",
        kind: "adult_video",
        include_adult: true,
      }),
    ),
  );
  expect(await screen.findByRole("link", { name: "Details" })).toHaveAttribute(
    "href",
    "/adult/media/$mediaId",
  );
});
it("uses catalog-specific metadata and requires separate merge and output confirmations", async () => {
  await candidate();
  expect(adultApi.search).toHaveBeenCalledWith(
    {
      catalog_number: "SYNTHETIC-001",
      namespace: "dmm",
      studio: null,
      language: "en-US",
    },
    ["dmm"],
  );
  expect(screen.getByText("Synthetic Series")).toBeInTheDocument();
  expect(screen.getByText(/Synthetic Alias/)).toBeInTheDocument();
  expect(
    screen.getByText("Covers and samples are hidden in privacy settings."),
  ).toBeInTheDocument();
  expect(adultApi.artworkPreview).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Local adult media"), {
    target: { value: "synthetic-media" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Preview adult metadata merge" }),
  );
  await screen.findByText("Protected local field");
  expect(adultApi.confirm).not.toHaveBeenCalled();
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm adult metadata merge" }),
  );
  await screen.findByText("Adult metadata saved");
  fireEvent.click(
    screen.getByRole("button", { name: "Preview adult sidecar output" }),
  );
  await screen.findByText("synthetic.nfo · nfo");
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and write adult sidecars" }),
  );
  await screen.findByText("Sidecar output recorded");
  expect(adultApi.outputExecute).toHaveBeenCalledWith("synthetic-output");
});
it("downloads only a requested validated thumbnail and keeps artwork selection independent", async () => {
  privacy.settings.covers_visible = true;
  await candidate();
  expect(adultApi.artworkPreview).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getAllByRole("button", { name: "Preview selected image" })[1],
  );
  await screen.findByRole("img", { name: "Cover and sample selection" });
  expect(adultApi.artworkPreview).toHaveBeenCalledWith("synthetic-run", 0, 1);
  for (const image of screen.getAllByRole("img"))
    expect(image.getAttribute("src")).toMatch(/^data:image\//);
});
it("previews a bounded batch in tools then cancels without executing output", async () => {
  show(<AdultToolsPage />);
  await screen.findByRole("checkbox", {
    name: "Select for batch output: Synthetic local adult fixture",
  });
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Select for batch output: Synthetic local adult fixture",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview batch output" }));
  await screen.findByText("synthetic.nfo · nfo");
  expect(adultApi.batchPreview).toHaveBeenCalledWith(["synthetic-media"]);
  expect(adultApi.batchExecute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel batch" }));
  await screen.findByText("Batch results: Cancelled");
  expect(adultApi.batchCancel).toHaveBeenCalledWith("synthetic-batch");
  expect(adultApi.batchExecute).not.toHaveBeenCalled();
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
});
it("executes a batch in tools only after the confirmation button", async () => {
  show(<AdultToolsPage />);
  await screen.findByRole("checkbox", {
    name: "Select for batch output: Synthetic local adult fixture",
  });
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Select for batch output: Synthetic local adult fixture",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview batch output" }));
  await screen.findByRole("button", { name: "Confirm and execute batch" });
  expect(adultApi.batchExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and execute batch" }),
  );
  await screen.findByText("Batch results: Completed");
  expect(adultApi.batchExecute).toHaveBeenCalledWith("synthetic-batch");
});
it("shows adult roots and scans only through the dedicated tools", async () => {
  show(<AdultToolsPage />);
  fireEvent.click(screen.getByRole("button", { name: "Library roots" }));
  await screen.findByText("No library roots yet");
  expect(adultApi.roots).toHaveBeenCalled();
  expect(libraryApi.roots).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "File scanner" }));
  await screen.findByLabelText("Choose an enabled root");
  expect(adultApi.tasks).toHaveBeenCalled();
  expect(libraryApi.tasks).not.toHaveBeenCalled();
});
it("renders the independent workflow in simplified Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  show(<AdultToolsPage />);
  await screen.findByText("成人工作台");
  fireEvent.click(screen.getByRole("button", { name: "刮削工作台" }));
  await screen.findByLabelText("番号");
  expect(
    screen.getByRole("button", { name: "搜索番号来源" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("番号命名空间")).toHaveValue("dmm");
  expect(document.body.textContent).not.toMatch(/adult[A-Z]|providerHealth_/);
});

it("shows and cancels adult provider tasks in the separate task view", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.tasks = [
    {
      id: "synthetic-run",
      provider_id: "dmm",
      query: "SYNTHETIC-001",
      state: "running",
      created_at: "2026-11-03T00:00:00Z",
      updated_at: "2026-11-03T00:00:00Z",
      failure: null,
      result_summary: null,
    },
  ];
  vi.mocked(adultApi.providers).mockResolvedValue(snapshot);
  vi.mocked(providerApi.cancel).mockResolvedValue(true);
  show(<AdultToolsPage />);
  fireEvent.click(screen.getByRole("button", { name: "Task center" }));
  await screen.findByText("dmm · SYNTHETIC-001");
  expect(libraryApi.tasks).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel task" }));
  await waitFor(() =>
    expect(providerApi.cancel).toHaveBeenCalledWith("synthetic-run"),
  );
});
it("displays recent visits only when the user enables that setting", async () => {
  privacy.settings.remember_recent = true;
  vi.mocked(adultApi.recent).mockResolvedValue([
    {
      media_id: "synthetic-media",
      title: "Synthetic recent visit",
      visited_at: "2026-11-03T00:00:00Z",
    },
  ]);
  show();
  await screen.findByRole("link", { name: "Synthetic recent visit" });
  expect(adultApi.recent).toHaveBeenCalledOnce();
});

it("keeps adult consumption read-only and links to separate tools", async () => {
  show();
  await screen.findByText("Synthetic local adult fixture");
  expect(
    screen.getByRole("button", {
      name: "Open in system app: Synthetic local adult fixture",
    }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Preview|Confirm|Search catalog/ }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Media tools" })).toHaveAttribute(
    "href",
    "/adult/tools",
  );
  expect(adultApi.batchPreview).not.toHaveBeenCalled();
  expect(adultApi.outputPreview).not.toHaveBeenCalled();
  expect(document.querySelector("audio,video")).toBeNull();
});
