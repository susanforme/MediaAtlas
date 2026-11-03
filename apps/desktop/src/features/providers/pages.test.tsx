import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { providerApi } from "../../api/providers";
import i18n from "../../i18n";
import { fixtureSnapshot, fixtureTask } from "./fixtures";
import { NetworkInspectorPage, ProvidersPage, ProviderTasks } from "./pages";

vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: {
    snapshot: vi.fn(),
    setEnabled: vi.fn(),
    updateConfig: vi.fn(),
    start: vi.fn(),
    cancel: vi.fn(),
  },
}));
function show(component: React.ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>{component}</QueryClientProvider>
    </I18nextProvider>,
  );
  return client;
}
beforeEach(async () => {
  await i18n.changeLanguage("en-US");
  vi.mocked(providerApi.snapshot).mockResolvedValue(fixtureSnapshot());
  vi.mocked(providerApi.setEnabled).mockResolvedValue(null);
  vi.mocked(providerApi.updateConfig).mockResolvedValue(null);
  vi.mocked(providerApi.start).mockResolvedValue("task-1");
  vi.mocked(providerApi.cancel).mockResolvedValue(true);
});
it("requires an explicit user action to run and disable the fixture", async () => {
  show(<ProvidersPage />);
  await screen.findByText("Offline dual-engine fixture");
  expect(providerApi.start).not.toHaveBeenCalled();
  expect(
    screen.getByRole("button", { name: "Run offline fixture" }),
  ).toBeDisabled();
  fireEvent.change(
    screen.getByLabelText("Test search text (do not enter credentials)"),
    { target: { value: " A title " } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Run offline fixture" }));
  await waitFor(() =>
    expect(providerApi.start).toHaveBeenCalledWith(
      "fixture.offline",
      "A title",
      "en-US",
    ),
  );
  await screen.findByText(
    "Task submitted. View its result in the task center.",
  );
  fireEvent.click(screen.getByLabelText("Enable provider"));
  await waitFor(() =>
    expect(providerApi.setEnabled).toHaveBeenCalledWith(
      "fixture.offline",
      false,
    ),
  );
});
it("validates editable limits before updating an immutable configuration snapshot", async () => {
  show(<ProvidersPage />);
  await screen.findByText("Offline dual-engine fixture");
  fireEvent.click(screen.getByText("Runtime configuration"));
  fireEvent.change(screen.getByLabelText("Concurrent requests"), {
    target: { value: "0" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await screen.findByRole("alert");
  expect(providerApi.updateConfig).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Concurrent requests"), {
    target: { value: "3" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() =>
    expect(providerApi.updateConfig).toHaveBeenCalledWith(
      "fixture.offline",
      expect.objectContaining({ concurrency: 3, revision: 1 }),
    ),
  );
});
it("shows persisted provider tasks and cancels only active tasks", async () => {
  const data = fixtureSnapshot();
  data.tasks = [
    fixtureTask,
    {
      ...fixtureTask,
      id: "done",
      state: "succeeded",
      result_summary: "fixture_complete",
    },
  ];
  vi.mocked(providerApi.snapshot).mockResolvedValue(data);
  show(<ProviderTasks />);
  await screen.findByText(
    "Offline JSON and HTML combination passed. No media was changed.",
  );
  expect(screen.getAllByRole("button", { name: "Cancel task" })).toHaveLength(
    1,
  );
  fireEvent.click(screen.getByRole("button", { name: "Cancel task" }));
  await waitFor(() =>
    expect(providerApi.cancel).toHaveBeenCalledWith(
      "task-1",
      expect.anything(),
    ),
  );
});
it("renders redacted request diagnostics and localized failure categories", async () => {
  const data = fixtureSnapshot();
  data.requests = [
    {
      task_id: "task-1",
      provider_id: "fixture.offline",
      engine: "api",
      method: "GET",
      safe_url: "https://example.test",
      duration_ms: 42,
      status: 429,
      failure: "rate_limited",
      summary: null,
      attempt: 2,
    },
  ];
  vi.mocked(providerApi.snapshot).mockResolvedValue(data);
  show(<NetworkInspectorPage />);
  await screen.findByText("Site rate limit");
  expect(screen.getByText("https://example.test")).toBeInTheDocument();
  expect(screen.getByText("42 ms")).toBeInTheDocument();
  expect(screen.getByText("429")).toBeInTheDocument();
});
it("shows desktop-required instead of fake data in a browser", async () => {
  vi.mocked(providerApi.snapshot).mockRejectedValue(
    new DesktopError("desktop_required"),
  );
  show(<ProvidersPage />);
  await screen.findByText("Open in the desktop app");
  expect(
    screen.queryByRole("button", { name: "Run offline fixture" }),
  ).not.toBeInTheDocument();
});
it("updates provider task history on polling without user navigation", async () => {
  vi.mocked(providerApi.snapshot)
    .mockResolvedValueOnce({ ...fixtureSnapshot(), tasks: [fixtureTask] })
    .mockResolvedValue({
      ...fixtureSnapshot(),
      tasks: [
        {
          ...fixtureTask,
          state: "succeeded",
          result_summary: "fixture_complete",
        },
      ],
    });
  show(<ProviderTasks />);
  await screen.findByText("Running");
  await screen.findByText(
    "Offline JSON and HTML combination passed. No media was changed.",
    {},
    { timeout: 3500 },
  );
});
it("localizes provider states and failure explanations in Simplified Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  const data = fixtureSnapshot();
  data.tasks = [{ ...fixtureTask, state: "failed", failure: "captcha" }];
  vi.mocked(providerApi.snapshot).mockResolvedValue(data);
  show(<ProviderTasks />);
  await screen.findByText("需要验证码，未尝试绕过");
  expect(screen.getByText("失败")).toBeInTheDocument();
});

it("snapshots the selected Chinese UI language when starting a provider task", async () => {
  await i18n.changeLanguage("zh-CN");
  show(<ProvidersPage />);
  await screen.findByText("离线双引擎 Fixture");
  fireEvent.change(screen.getByLabelText("测试检索文本（请勿输入凭据）"), {
    target: { value: "中文检索" },
  });
  fireEvent.click(screen.getByRole("button", { name: "运行离线 Fixture" }));
  await waitFor(() =>
    expect(providerApi.start).toHaveBeenCalledWith(
      "fixture.offline",
      "中文检索",
      "zh-CN",
    ),
  );
});
