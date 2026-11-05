import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { providerApi } from "../../api/providers";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { fixtureSnapshot } from "./fixtures";
import { NetworkInspectorPage, ProvidersPage } from "./pages";

vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    children,
    className,
  }: {
    to: string;
    search?: Record<string, string | string[] | undefined>;
    children: React.ReactNode;
    className?: string;
  }) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).flatMap(([key, value]) =>
        value === undefined
          ? []
          : [[key, typeof value === "string" ? value : JSON.stringify(value)]],
      ),
    ).toString();
    return (
      <a href={to + (query ? `?${query}` : "")} className={className}>
        {children}
      </a>
    );
  },
}));
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
  await screen.findByText("Offline demo");
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
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Task submitted. View its result in the task center.",
    ),
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
  await screen.findByText("Offline demo");
  fireEvent.click(screen.getByText("Request limits"));
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
      expect.objectContaining({
        concurrency: 3,
        revision: 1,
        credential_ref: null,
      }),
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

it("snapshots the selected Chinese UI language when starting a provider task", async () => {
  await i18n.changeLanguage("zh-CN");
  show(<ProvidersPage />);
  await screen.findByText("离线演示");
  fireEvent.change(screen.getByLabelText("测试检索文本（请勿输入凭据）"), {
    target: { value: "中文检索" },
  });
  fireEvent.click(screen.getByRole("button", { name: "运行离线演示" }));
  await waitFor(() =>
    expect(providerApi.start).toHaveBeenCalledWith(
      "fixture.offline",
      "中文检索",
      "zh-CN",
    ),
  );
});
it("routes real providers to Scraper and Settings without an offline fixture search form", async () => {
  const data = fixtureSnapshot();
  data.providers[0].descriptor = {
    ...data.providers[0].descriptor,
    id: "tmdb",
    name: "TMDB",
    fixture: false,
    capabilities: ["details", "images", "external_ids", "article_lookup"],
  };
  vi.mocked(providerApi.snapshot).mockResolvedValue(data);
  show(<ProvidersPage />);
  await screen.findByText("TMDB");
  expect(
    screen.queryByRole("button", { name: "Run offline fixture" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByLabelText("Test search text (do not enter credentials)"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Search in Scraper" }),
  ).toHaveAttribute("href", "/scraper");
  expect(
    screen.getByRole("link", { name: "Save authentication in Settings" }),
  ).toHaveAttribute("href", "/settings?provider=tmdb&tab=auth");
  expect(
    screen.getByText("Details · Images · External IDs · Article lookup"),
  ).toBeInTheDocument();
  expect(
    screen.queryByLabelText("Credential reference (not a secret)"),
  ).not.toBeInTheDocument();
  expect(providerApi.start).not.toHaveBeenCalled();
});

it("never displays an opaque credential reference while editing limits", async () => {
  const data = fixtureSnapshot();
  data.providers[0].config.credential_ref = "opaque-backend-owned-reference";
  vi.mocked(providerApi.snapshot).mockResolvedValue(data);
  show(<ProvidersPage />);
  await screen.findByText("Offline demo");
  fireEvent.click(screen.getByText("Request limits"));
  expect(
    screen.queryByDisplayValue("opaque-backend-owned-reference"),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText("opaque-backend-owned-reference"),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() =>
    expect(providerApi.updateConfig).toHaveBeenCalledWith(
      "fixture.offline",
      expect.objectContaining({ credential_ref: null }),
    ),
  );
});

it("returns to the fixed scraper route with the originating local media", async () => {
  show(<ProvidersPage returnToScraper returnMediaId="media-1" />);
  expect(
    screen.getByRole("link", { name: "Return to scraper" }),
  ).toHaveAttribute("href", "/scraper?media=media-1");
  await screen.findByText("Offline demo");
});

it("keeps scraper context when continuing from providers to authentication settings", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.fixture = false;
  snapshot.providers[0].descriptor.id = "tmdb";
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  show(<ProvidersPage returnToScraper returnMediaId="media-1" />);
  expect(
    await screen.findByRole("link", {
      name: "Save authentication in Settings",
    }),
  ).toHaveAttribute(
    "href",
    "/settings?from=scraper&media=media-1&provider=tmdb&tab=auth",
  );
  expect(
    screen.getByRole("link", { name: "Search in Scraper" }),
  ).toHaveAttribute("href", "/scraper?media=media-1");
});

it("preserves review order in provider return and authentication continuation links", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.fixture = false;
  snapshot.providers[0].descriptor.id = "tmdb";
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  show(
    <ProvidersPage
      returnToScraper
      returnMediaId="media-2"
      returnQueue={["media-1", "media-2"]}
    />,
  );
  await screen.findByRole("link", { name: "Save authentication in Settings" });
  for (const name of [
    "Return to scraper",
    "Search in Scraper",
    "Save authentication in Settings",
  ]) {
    const url = new URL(
      screen.getByRole("link", { name }).getAttribute("href") ?? "",
      "https://example.test",
    );
    expect(url.searchParams.get("media")).toBe("media-2");
    expect(JSON.parse(url.searchParams.get("queue") ?? "null")).toEqual([
      "media-1",
      "media-2",
    ]);
    if (name === "Save authentication in Settings")
      expect(url.searchParams.get("from")).toBe("scraper");
  }
});

it("keeps a durable review session through source and authentication links", async () => {
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.fixture = false;
  snapshot.providers[0].descriptor.id = "tmdb";
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  const session = "11111111-1111-4111-8111-111111111111";
  show(
    <ProvidersPage
      embedded
      setup={{
        from: "scraper",
        session,
        media: "22222222-2222-4222-8222-222222222222",
      }}
    />,
  );
  await screen.findByRole("link", { name: "Save authentication in Settings" });
  for (const name of ["Search in Scraper", "Save authentication in Settings"]) {
    const url = new URL(
      screen.getByRole("link", { name }).getAttribute("href") ?? "",
      "https://example.test",
    );
    expect(url.searchParams.get("session")).toBe(session);
    expect(url.searchParams.get("media")).toBe(
      "22222222-2222-4222-8222-222222222222",
    );
  }
});
