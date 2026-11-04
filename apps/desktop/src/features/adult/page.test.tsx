import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { type AdultSettings, adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import { providerApi } from "../../api/providers";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { fixtureSnapshot } from "../providers/fixtures";
import {
  syntheticAdultDetail,
  syntheticAdultMedia,
  syntheticAdultMerge,
  syntheticAdultOutput,
  syntheticAdultSearch,
} from "./fixtures";
import { AdultLibraryPage } from "./library-page";
import {
  AdultBatchOutput,
  AdultMediaPage,
  AdultOutput,
  AdultPage,
  AdultScraper,
  AdultTasksPage,
} from "./page";
import { AdultRootsPage, AdultScannerPage } from "./resource-pages";

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
vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));
vi.mock("../library/context", () => ({ usePrivacy: () => privacy }));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    params,
    search,
    children,
    onClick,
    ...props
  }: ComponentProps<"a"> & {
    to: string;
    params?: { mediaId: string };
    search?: Record<string, unknown>;
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
vi.mock("../../api/organization", async (original) => ({
  ...(await original<typeof import("../../api/organization")>()),
  nativeOpenApi: { open: vi.fn() },
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
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
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
  show(
    <>
      <AdultScraper />
      <AdultOutput mediaId="synthetic-media" />
    </>,
  );
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
it("waits for backend readiness without exposing an unlock workflow or requesting resource data", () => {
  privacy.includeAdult = false;
  privacy.settings.unlocked = false;
  show();
  expect(
    screen.queryByRole("button", { name: /Unlock|Lock|解锁|锁定/ }),
  ).not.toBeInTheDocument();
  expect(screen.queryByText(/workspace locked/)).not.toBeInTheDocument();
  expect(adultApi.media).not.toHaveBeenCalled();
  expect(adultApi.providers).not.toHaveBeenCalled();
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
  fireEvent.change(screen.getByRole("searchbox", { name: "Search titles" }), {
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
  const poster = await screen.findByRole("link", {
    name: "Details: Synthetic local adult fixture",
  });
  expect(poster).toHaveAttribute(
    "href",
    "/media/synthetic-media?kind=adult_video",
  );
  expect(poster).toHaveTextContent("Synthetic local adult fixture");
  expect(poster.querySelector(".poster-image")).not.toBeNull();
  expect(libraryApi.detail).not.toHaveBeenCalled();
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
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
  expect(screen.queryByText(/privacy settings/)).not.toBeInTheDocument();
  expect(adultApi.artworkPreview).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Local media"), {
    target: { value: "synthetic-media" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Preview metadata changes" }),
  );
  await screen.findByText("Protected local field");
  expect(adultApi.confirm).not.toHaveBeenCalled();
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Save metadata" }));
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith("Metadata saved"),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Preview sidecar output" }),
  );
  await screen.findByText("synthetic.nfo · nfo");
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and write sidecars" }),
  );
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith("Sidecar output recorded"),
  );
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
  show(<AdultBatchOutput mediaIds={["synthetic-media"]} />);
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
  show(<AdultBatchOutput mediaIds={["synthetic-media"]} />);
  fireEvent.click(screen.getByRole("button", { name: "Preview batch output" }));
  await screen.findByRole("button", { name: "Confirm and execute batch" });
  expect(adultApi.batchExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and execute batch" }),
  );
  await screen.findByText("Batch results: Completed");
  expect(adultApi.batchExecute).toHaveBeenCalledWith("synthetic-batch");
});
it("uses the shared directory and scanner pages while keeping adult queries isolated", async () => {
  const roots = show(<AdultRootsPage />);
  await screen.findByText("No library roots yet");
  expect(adultApi.roots).toHaveBeenCalled();
  expect(libraryApi.roots).toHaveBeenCalledWith(false);
  roots.unmount();
  const onAddRoot = vi.fn();
  show(
    <AdultScannerPage
      search={{ kind: "adult_video" }}
      onSearch={() => {}}
      onAddRoot={onAddRoot}
    />,
  );
  const addDirectory = await screen.findByRole("button", {
    name: "Add media directory",
  });
  expect(adultApi.tasks).toHaveBeenCalled();
  expect(libraryApi.tasks).toHaveBeenCalledWith(false);
  fireEvent.click(addDirectory);
  expect(onAddRoot).toHaveBeenCalledOnce();
});
it("renders type-specific tools in the shared interface in simplified Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  show(<AdultScraper />);
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
  show(<AdultTasksPage />);
  await screen.findByText("dmm · SYNTHETIC-001");
  expect(libraryApi.tasks).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel task" }));
  await waitFor(() =>
    expect(providerApi.cancel).toHaveBeenCalledWith("synthetic-run"),
  );
});
it("does not introduce a type-specific recent visits or lock panel", async () => {
  privacy.settings.remember_recent = true;
  show();
  await screen.findByText("Synthetic local adult fixture");
  expect(adultApi.recent).not.toHaveBeenCalled();
  expect(
    screen.queryByRole("button", { name: /Unlock|Lock/ }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText(/recent visits|workspace/i),
  ).not.toBeInTheDocument();
});

it("keeps adult consumption read-only and links to separate tools", async () => {
  show();
  await screen.findByText("Synthetic local adult fixture");
  const poster = screen.getByRole("link", {
    name: "Details: Synthetic local adult fixture",
  });
  expect(poster).toHaveAttribute(
    "href",
    "/media/synthetic-media?kind=adult_video",
  );
  poster.focus();
  expect(poster).toHaveFocus();
  fireEvent.click(poster);
  expect(nativeOpenApi.open).not.toHaveBeenCalled();
  expect(
    screen.queryByRole("button", { name: /Open in system app/ }),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Preview|Confirm|Search catalog/ }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Media tools" })).toHaveAttribute(
    "href",
    "/tools?kind=adult_video",
  );
  expect(libraryApi.media).not.toHaveBeenCalled();
  expect(libraryApi.detail).not.toHaveBeenCalled();
  expect(adultApi.confirm).not.toHaveBeenCalled();
  expect(adultApi.outputExecute).not.toHaveBeenCalled();
  expect(adultApi.batchExecute).not.toHaveBeenCalled();
  expect(adultApi.batchPreview).not.toHaveBeenCalled();
  expect(adultApi.outputPreview).not.toHaveBeenCalled();
  expect(document.querySelector("audio,video")).toBeNull();
});

it("presents adult content as a type in the shared library and clears private filters when changing type", async () => {
  const onSearch = vi.fn();
  show(
    <AdultLibraryPage
      search={{
        kind: "adult_video",
        root: "adult-root",
        genre: "private-genre",
        cursor: "private-cursor",
      }}
      onSearch={onSearch}
    />,
  );
  await screen.findByText("Synthetic local adult fixture");
  expect(
    screen.getByRole("heading", { name: "Media library" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: "Adult library" }),
  ).not.toBeInTheDocument();
  const type = screen.getByRole("combobox", { name: "Media type" });
  expect(type).toHaveValue("adult_video");
  fireEvent.change(type, { target: { value: "movie" } });
  expect(onSearch).toHaveBeenCalledWith(
    expect.objectContaining({
      kind: "movie",
      root: undefined,
      genre: undefined,
      cursor: undefined,
    }),
  );
  expect(libraryApi.media).not.toHaveBeenCalled();
});

it("uses the selected resource and its parsed catalog without asking the user to choose the target again", async () => {
  vi.mocked(adultApi.toolsDetail).mockResolvedValue({
    ...syntheticAdultMedia.items[0],
    files: [],
    artworks: [],
    external_ids: [],
    relations: [],
    metadata_values: [
      {
        id: "catalog-field",
        field_key: "catalog_number",
        locale: null,
        value: "SYNTHETIC-001",
        provider_id: "local",
        source_url: null,
        is_user_modified: false,
        is_locked: false,
      },
    ],
  });
  show(<AdultScraper mediaId="synthetic-media" />);
  await waitFor(() =>
    expect(screen.getByLabelText("Catalog number")).toHaveValue(
      "SYNTHETIC-001",
    ),
  );
  fireEvent.click(screen.getByRole("checkbox", { name: "DMM / FANZA" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Search catalog providers" }),
  );
  await screen.findByText("Catalog number matches");
  fireEvent.click(screen.getByRole("button", { name: "Review candidate" }));
  await screen.findByText("Synthetic Label");
  expect(
    screen.queryByRole("combobox", { name: "Local media" }),
  ).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Preview metadata changes" }),
  );
  await waitFor(() =>
    expect(adultApi.preview).toHaveBeenCalledWith(
      expect.objectContaining({ media_item_id: "synthetic-media" }),
    ),
  );
  expect(adultApi.toolsMedia).not.toHaveBeenCalled();
  expect(adultApi.confirm).not.toHaveBeenCalled();
});

it("does not notify an output result after the private workspace unmounts", async () => {
  let finish!: (
    value: Awaited<ReturnType<typeof adultApi.outputExecute>>,
  ) => void;
  vi.mocked(adultApi.outputExecute).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const view = show(<AdultOutput mediaId="synthetic-media" />);
  fireEvent.click(
    screen.getByRole("button", { name: "Preview sidecar output" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Confirm and write sidecars" }),
  );
  await waitFor(() =>
    expect(adultApi.outputExecute).toHaveBeenCalledWith("synthetic-output"),
  );
  view.unmount();
  await act(async () => {
    finish({
      state: "succeeded",
      schema_version: 1,
      preview_id: "synthetic-output",
      written: 1,
      preserved: 0,
      already_applied: false,
    });
  });
  expect(toast.success).not.toHaveBeenCalled();
});
