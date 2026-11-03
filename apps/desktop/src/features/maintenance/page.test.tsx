import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { databaseMaintenanceApi as api } from "../../api/maintenance";
import i18n from "../../i18n";
import { MaintenancePage } from "./page";

vi.mock("../../api/maintenance", () => ({
  databaseMaintenanceApi: {
    status: vi.fn(),
    backupPreview: vi.fn(),
    backupConfirm: vi.fn(),
    restorePreview: vi.fn(),
    restoreConfirm: vi.fn(),
    cachePreview: vi.fn(),
    cacheConfirm: vi.fn(),
    diagnostics: vi.fn(),
    cancel: vi.fn(),
  },
}));
const privacy = vi.hoisted(() => ({ includeAdult: false }));
vi.mock("../library/context", () => ({ usePrivacy: () => privacy }));
const summary = {
  id: "backup",
  created_at: "2026-11-03",
  app_version: "0.1.0-beta.1",
  schema_version: 17,
  bytes: 4096,
  contains_private_metadata: false,
};
function view() {
  return (
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <MaintenancePage />
    </QueryClientProvider>
  );
}
beforeEach(async () => {
  vi.resetAllMocks();
  privacy.includeAdult = false;
  await i18n.changeLanguage("en-US");
  vi.mocked(api.status).mockResolvedValue({
    backups: [summary],
    restore_pending: false,
    directory: "/synthetic/maintenance",
  });
  vi.mocked(api.backupPreview).mockResolvedValue({
    id: "preview",
    contains_private_metadata: false,
    includes_secrets: false,
    includes_source_media: false,
    may_include_tag_recovery_bytes: true,
  });
  vi.mocked(api.backupConfirm).mockResolvedValue(summary);
  vi.mocked(api.restorePreview).mockResolvedValue({
    id: "restore-preview",
    backup_id: "backup",
    contains_private_metadata: false,
    restart_required: true,
  });
  vi.mocked(api.restoreConfirm).mockResolvedValue({ restart_required: true });
});
it("previews before explicit backup confirmation and makes no startup writes", async () => {
  render(view());
  await screen.findByText("2026-11-03 · v0.1.0-beta.1 · 4096 B");
  expect(api.backupPreview).not.toHaveBeenCalled();
  expect(api.backupConfirm).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Preview full database backup" }),
  );
  await screen.findByRole("button", { name: "Confirm database backup" });
  expect(api.backupConfirm).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm database backup" }),
  );
  await waitFor(() =>
    expect(api.backupConfirm).toHaveBeenCalledWith("preview"),
  );
  await screen.findByText("Backup saved.");
});
it("warns about restart and safety snapshot before queuing restore", async () => {
  render(view());
  fireEvent.click(
    await screen.findByRole("button", { name: "Preview restore" }),
  );
  await screen.findByText(
    /A safety snapshot of the current database is preserved first/,
  );
  expect(api.restoreConfirm).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm restore at next startup" }),
  );
  await waitFor(() =>
    expect(api.restoreConfirm).toHaveBeenCalledWith("restore-preview"),
  );
  await screen.findByText(/Restore queued/);
});
it("discards privacy-bound previews after locking", async () => {
  privacy.includeAdult = true;
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const content = () => (
    <QueryClientProvider client={client}>
      <MaintenancePage />
    </QueryClientProvider>
  );
  const mounted = render(content());
  fireEvent.click(
    await screen.findByRole("button", { name: "Preview full database backup" }),
  );
  await screen.findByRole("button", { name: "Confirm database backup" });
  privacy.includeAdult = false;
  mounted.rerender(content());
  expect(
    screen.queryByRole("button", { name: "Confirm database backup" }),
  ).not.toBeInTheDocument();
  expect(api.backupConfirm).not.toHaveBeenCalled();
});

it("does not reveal a diagnostic result that arrives after privacy is locked", async () => {
  privacy.includeAdult = true;
  let complete!: (result: Awaited<ReturnType<typeof api.diagnostics>>) => void;
  vi.mocked(api.diagnostics).mockImplementation(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const content = () => (
    <QueryClientProvider client={client}>
      <MaintenancePage />
    </QueryClientProvider>
  );
  const mounted = render(content());
  fireEvent.click(
    await screen.findByRole("button", { name: "Export safe diagnostics" }),
  );
  expect(api.diagnostics).toHaveBeenCalledOnce();
  privacy.includeAdult = false;
  mounted.rerender(content());
  await act(async () => {
    complete({
      id: "diagnostics",
      path: "/synthetic/private-diagnostics.txt",
      bytes: 128,
    });
  });
  expect(
    screen.queryByDisplayValue("/synthetic/private-diagnostics.txt"),
  ).not.toBeInTheDocument();
  expect(screen.queryByText("Diagnostics saved")).not.toBeInTheDocument();
});
