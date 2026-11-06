import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type RootView } from "../../api/library";
import { maintenanceApi, type OrganizationPlan } from "../../api/organization";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { chooseSelectOption, selectOptions } from "../../test/select";
import { MediaToolsPage, OrganizationPanel } from "./page";

const api = vi.hoisted(() => ({
  preview: vi.fn(),
  execute: vi.fn(),
  cancel: vi.fn(),
  discard: vi.fn(),
  rollbackPreview: vi.fn(),
  rollbackExecute: vi.fn(),
  history: vi.fn(),
  logs: vi.fn(),
  plan: vi.fn(),
}));
vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  Link: ({ to, search, children }: { to: string; search?: Record<string, unknown>; children: React.ReactNode }) => (
    <a href={`${to}${search?.media ? `?media=${encodeURIComponent(String(search.media))}` : ""}`}>{children}</a>
  ),
}));
vi.mock("../../api/organization", async (original) => ({
  ...(await original<typeof import("../../api/organization")>()),
  organizationApi: () => api,
  maintenanceApi: { watchers: vi.fn(), configure: vi.fn(), health: vi.fn() },
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { roots: vi.fn(), toolsMedia: vi.fn(), toolsDetail: vi.fn() },
}));
const root: RootView = {
  id: "root",
  name: "Target",
  path: "/fixture",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  created_at: "2026-01-01",
  updated_at: "2026-01-01",
  exclude_patterns: [],
  last_scan_at: null,
};
const plan: OrganizationPlan = {
  id: "plan",
  media_item_id: "media",
  target_root_id: "root",
  strategy: "preserve",
  state: "planned",
  files: [
    {
      source: "source/Part1.mkv",
      target: "target/Part1.mkv",
      role: "media",
      bytes: 7,
      conflict: null,
      same_path: false,
    },
    {
      source: "source/Part1.en.srt",
      target: "target/Part1.en.srt",
      role: "subtitle",
      bytes: 3,
      conflict: null,
      same_path: false,
    },
  ],
  estimated_bytes: 10,
  file_count: 2,
  warnings: [],
};
const result = {
  id: "plan",
  state: "succeeded",
  completed: 2,
  already_applied: false,
  error_code: null,
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
beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([root]);
  vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
    items: [],
    next_cursor: null,
  });
  api.preview.mockResolvedValue(plan);
  api.execute.mockResolvedValue(result);
  api.history.mockResolvedValue([]);
  api.discard.mockResolvedValue(null);
  api.rollbackPreview.mockResolvedValue({ ...plan, id: "rollback" });
  api.rollbackExecute.mockResolvedValue({ ...result, id: "rollback" });
  api.logs.mockResolvedValue([
    { file_index: 0, stage: "published", created_at: "2026-01-01" },
  ]);
  api.plan.mockResolvedValue(plan);
  vi.mocked(maintenanceApi.watchers).mockResolvedValue([
    {
      root_id: "root",
      opted_in: false,
      active: false,
      pending_rescan: false,
      last_task_id: null,
      error_code: null,
    },
  ]);
  vi.mocked(maintenanceApi.health).mockResolvedValue({
    items: [
      {
        media_id: "media",
        title: "Synthetic issue",
        issues: ["offline_file"],
        repair_route: "/tools?media=media",
      },
    ],
    next_cursor: null,
  });
});
it("defaults to Preserve, shows every attachment and requires explicit plan confirmation", async () => {
  show(<OrganizationPanel mediaId="media" />);
  selectOptions(screen.getByRole("combobox", { name: "Destination root" }));
  await screen.findByRole("option", { name: "Target" });
  expect(screen.getByRole("combobox", { name: "Strategy" })).toHaveValue(
    "preserve",
  );
  chooseSelectOption(
    screen.getByRole("combobox", { name: "Destination root" }),
    "root",
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview organization" }));
  await screen.findByText("source/Part1.en.srt → target/Part1.en.srt");
  expect(api.execute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm and execute this plan" }),
  );
  await waitFor(() => expect(api.execute).toHaveBeenCalledWith("plan"));
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith("Operation result: Completed"),
  );
  expect(api.preview).toHaveBeenCalledWith(
    expect.objectContaining({
      media_item_id: "media",
      strategy: "preserve",
      target_root_id: "root",
    }),
  );
});
it("blocks conflicts and discards a preview when naming inputs change", async () => {
  api.preview.mockResolvedValue({
    ...plan,
    files: [{ ...plan.files[0], conflict: "destination_exists" }],
  });
  show(<OrganizationPanel mediaId="media" />);
  selectOptions(screen.getByRole("combobox", { name: "Destination root" }));
  await screen.findByRole("option", { name: "Target" });
  chooseSelectOption(
    screen.getByRole("combobox", { name: "Destination root" }),
    "root",
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview organization" }));
  await screen.findByText("Conflict: destination_exists");
  expect(
    screen.getByRole("button", { name: "Confirm and execute this plan" }),
  ).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Naming template"), {
    target: { value: "{stem}.{ext}" },
  });
  expect(
    screen.queryByRole("button", { name: "Confirm and execute this plan" }),
  ).not.toBeInTheDocument();
  expect(api.discard).toHaveBeenCalledWith("plan");
  expect(api.execute).not.toHaveBeenCalled();
});
it("shows persistent stage logs and gives rollback a separate Preview and confirmation", async () => {
  api.history.mockResolvedValue([result]);
  show(<OrganizationPanel mediaId="media" />);
  fireEvent.click(
    await screen.findByRole("button", { name: "View plan and step log" }),
  );
  await screen.findByText(/2026-01-01 · 0 · Destination published/);
  expect(api.logs).toHaveBeenCalledWith("plan");
  fireEvent.click(screen.getByRole("button", { name: "Preview rollback" }));
  await screen.findByRole("button", { name: "Confirm this rollback" });
  expect(api.rollbackExecute).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm this rollback" }),
  );
  await waitFor(() =>
    expect(api.rollbackExecute).toHaveBeenCalledWith("rollback"),
  );
});
it("saves the explicit watching choice and checks health only on request", async () => {
  vi.mocked(maintenanceApi.configure).mockResolvedValue({
    root_id: "root",
    opted_in: true,
    active: true,
    pending_rescan: false,
    last_task_id: null,
    error_code: null,
  });
  show(<MediaToolsPage search={{ view: "health" }} />);
  const checkbox = await screen.findByRole("checkbox", {
    name: "Target",
  });
  expect(screen.getByRole("heading", { level: 1, name: "Checks and monitoring" })).toBeInTheDocument();
  expect(screen.getByText("Not checked yet")).toBeInTheDocument();
  expect(checkbox).not.toBeChecked();
  expect(maintenanceApi.configure).not.toHaveBeenCalled();
  expect(maintenanceApi.health).not.toHaveBeenCalled();
  fireEvent.click(checkbox);
  await waitFor(() =>
    expect(maintenanceApi.configure).toHaveBeenCalledWith("root", true),
  );
  fireEvent.click(screen.getByRole("button", { name: "Check library health" }));
  await screen.findByText("Synthetic issue");
  expect(screen.getByText("Offline file")).toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Review in media tools" }),
  ).toHaveAttribute("href", "/tools?media=media");
  expect(api.execute).not.toHaveBeenCalled();
});

it("distinguishes an unchecked library, a running check and an empty completed batch", async () => {
  let finish!: (value: { items: []; next_cursor: null }) => void;
  vi.mocked(maintenanceApi.health).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
  show(<MediaToolsPage search={{ view: "health" }} />);
  await screen.findByRole("checkbox", { name: "Target" });
  const section = screen.getByRole("region", { name: "Library health" });
  expect(within(section).getByRole("status")).toHaveTextContent("Not checked yet");
  fireEvent.click(within(section).getByRole("button", { name: "Check library health" }));
  expect(section).toHaveAttribute("aria-busy", "true");
  expect(within(section).getByRole("status")).toHaveTextContent("Checking library…");
  expect(within(section).getByRole("button", { name: "Check library health" })).toBeDisabled();
  await act(async () => finish({ items: [], next_cursor: null }));
  await waitFor(() => expect(within(section).getByRole("status")).toHaveTextContent("No health findings in this batch"));
  expect(section).toHaveAttribute("aria-busy", "false");
  expect(maintenanceApi.health).toHaveBeenCalledTimes(1);
  expect(api.execute).not.toHaveBeenCalled();
});

it("retries the failed health-results batch only after an explicit retry", async () => {
  vi.mocked(maintenanceApi.health)
    .mockResolvedValueOnce({ items: [], next_cursor: "next-health-batch" })
    .mockRejectedValueOnce(new Error("private diagnostic detail"))
    .mockResolvedValueOnce({ items: [], next_cursor: null });
  show(<MediaToolsPage search={{ view: "health" }} />);
  fireEvent.click(screen.getByRole("button", { name: "Check library health" }));
  fireEvent.click(await screen.findByRole("button", { name: "More findings" }));
  await screen.findByRole("alert");
  expect(screen.queryByText("private diagnostic detail")).not.toBeInTheDocument();
  expect(maintenanceApi.health).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "Check again" }));
  await waitFor(() => expect(maintenanceApi.health).toHaveBeenCalledTimes(3));
  expect(vi.mocked(maintenanceApi.health).mock.calls.map(([cursor]) => cursor)).toEqual([null, "next-health-batch", "next-health-batch"]);
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  expect(api.execute).not.toHaveBeenCalled();
});

it("provides directory management when no folders can be watched", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValueOnce([]);
  vi.mocked(maintenanceApi.watchers).mockResolvedValueOnce([]);
  show(<MediaToolsPage search={{ view: "health" }} />);
  expect(await screen.findByText("No folders available for watching")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Library roots" })).toHaveAttribute("href", "/roots");
  expect(maintenanceApi.configure).not.toHaveBeenCalled();
  expect(maintenanceApi.health).not.toHaveBeenCalled();
});

it("freezes target and other planning actions while Preview or rollback Preview is pending", async () => {
  let resolve!: (value: OrganizationPlan) => void;
  api.history.mockResolvedValue([result]);
  api.preview.mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  show(<OrganizationPanel mediaId="media" />);
  selectOptions(screen.getByRole("combobox", { name: "Destination root" }));
  await screen.findByRole("option", { name: "Target" });
  chooseSelectOption(
    screen.getByRole("combobox", { name: "Destination root" }),
    "root",
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview organization" }));
  await waitFor(() => expect(api.preview).toHaveBeenCalled());
  expect(
    screen.getByRole("combobox", { name: "Destination root" }),
  ).toBeDisabled();
  expect(screen.getByLabelText("Naming template")).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "Strategy" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Preview rollback" }),
  ).toBeDisabled();
  await act(async () => resolve(plan));
  await screen.findByRole("button", { name: "Confirm and execute this plan" });
  let restore!: (value: OrganizationPlan) => void;
  api.rollbackPreview.mockReturnValue(
    new Promise((r) => {
      restore = r;
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview rollback" }));
  await waitFor(() => expect(api.rollbackPreview).toHaveBeenCalledWith("plan"));
  expect(
    screen.getByRole("combobox", { name: "Destination root" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Preview organization" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Preview rollback" }),
  ).toBeDisabled();
  expect(
    screen.queryByRole("button", { name: "Confirm and execute this plan" }),
  ).not.toBeInTheDocument();
  await act(async () => restore({ ...plan, id: "rollback" }));
  await screen.findByRole("button", { name: "Confirm this rollback" });
  expect(api.execute).not.toHaveBeenCalled();
  expect(api.rollbackExecute).not.toHaveBeenCalled();
});
it("discards a late Preview response after the tools view closes", async () => {
  let resolve!: (value: OrganizationPlan) => void;
  api.preview.mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  const view = show(<OrganizationPanel mediaId="media" />);
  selectOptions(screen.getByRole("combobox", { name: "Destination root" }));
  await screen.findByRole("option", { name: "Target" });
  chooseSelectOption(
    screen.getByRole("combobox", { name: "Destination root" }),
    "root",
  );
  fireEvent.click(screen.getByRole("button", { name: "Preview organization" }));
  await waitFor(() => expect(api.preview).toHaveBeenCalled());
  view.unmount();
  await act(async () => resolve(plan));
  expect(api.discard).toHaveBeenCalledWith("plan");
  expect(api.execute).not.toHaveBeenCalled();
});

it("does not expose a private item returned to the ordinary local workflow", async () => {
  const media = {
    id: "private",
    kind: "adult_video" as const,
    title: "PRIVATE TITLE",
    original_title: null,
    year: 2024,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  };
  vi.mocked(libraryApi.toolsMedia).mockResolvedValue({
    items: [{ media, poster_uri: null, health: [] }],
    next_cursor: null,
  });
  vi.mocked(libraryApi.toolsDetail).mockResolvedValue({
    media,
    files: [
      {
        id: "file",
        media_item_id: "private",
        root_id: "root",
        relative_path: "PRIVATE FILE.mkv",
        size_bytes: 1,
        modified_at: null,
        part: null,
      },
    ],
    artworks: [],
    external_ids: [],
    metadata_values: [],
    relations: [],
    health: [],
    poster_uri: null,
  });
  show(<MediaToolsPage mediaId="private" />);
  await screen.findByRole("alert");
  expect(screen.queryByText("PRIVATE TITLE")).not.toBeInTheDocument();
  expect(screen.queryByText("PRIVATE FILE.mkv")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Scrape this media" }),
  ).not.toBeInTheDocument();
  expect(api.history).not.toHaveBeenCalled();
});
