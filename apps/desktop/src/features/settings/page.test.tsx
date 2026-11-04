import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import i18n from "../../i18n";
import { SettingsPage } from "./page";

vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    children,
  }: {
    to: string;
    search?: Record<string, string | string[] | undefined>;
    children: React.ReactNode;
  }) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).flatMap(([key, value]) =>
        value === undefined
          ? []
          : [[key, typeof value === "string" ? value : JSON.stringify(value)]],
      ),
    ).toString();
    return <a href={to + (query ? `?${query}` : "")}>{children}</a>;
  },
}));
vi.mock("./provider-auth", () => ({
  ProviderAuthSettings: () => <section aria-label="Authentication" />,
}));
vi.mock("../providers/pages", () => ({
  ProvidersPage: () => <section aria-label="Provider management" />,
  NetworkInspectorPage: () => <section aria-label="Network records" />,
}));
vi.mock("../maintenance/page", () => ({
  MaintenancePage: () => <section aria-label="Diagnostics" />,
}));
beforeEach(async () => {
  await i18n.changeLanguage("en-US");
});
it("returns to the originating media without accepting an arbitrary destination", () => {
  render(<SettingsPage returnToScraper returnMediaId="media-1" />);
  expect(
    screen.getByRole("link", { name: "Return to scraper" }),
  ).toHaveAttribute("href", "/scraper?media=media-1");
});
it("provides a fixed anonymous scraper return and hides it outside the workflow", () => {
  const view = render(<SettingsPage returnToScraper />);
  expect(
    screen.getByRole("link", { name: "Return to scraper" }),
  ).toHaveAttribute("href", "/scraper");
  view.rerender(<SettingsPage />);
  expect(
    screen.queryByRole("link", { name: "Return to scraper" }),
  ).not.toBeInTheDocument();
});

it("preserves the ordered review queue when returning from credentials", () => {
  render(
    <SettingsPage
      returnToScraper
      returnMediaId="media-2"
      returnQueue={["media-1", "media-2"]}
    />,
  );
  const url = new URL(
    screen
      .getByRole("link", { name: "Return to scraper" })
      .getAttribute("href") ?? "",
    "https://example.test",
  );
  expect(url.pathname).toBe("/scraper");
  expect(url.searchParams.get("media")).toBe("media-2");
  expect(JSON.parse(url.searchParams.get("queue") ?? "null")).toEqual([
    "media-1",
    "media-2",
  ]);
});

it("hosts management pages as keyboard-accessible tabs without loading inactive panels", async () => {
  const change = vi.fn();
  render(<SettingsPage onTabChange={change} />);
  const general = screen.getByRole("tab", { name: "General" });
  expect(general).toHaveAttribute("tabindex", "0");
  expect(general).toHaveAttribute(
    "aria-controls",
    screen.getByRole("tabpanel", { name: "General" }).id,
  );
  expect(
    screen.queryByRole("region", { name: "Authentication" }),
  ).not.toBeInTheDocument();
  general.focus();
  fireEvent.keyDown(general, { key: "ArrowRight" });
  const auth = screen.getByRole("tab", { name: "Authentication" });
  expect(auth).toHaveFocus();
  expect(auth).toHaveAttribute("aria-selected", "true");
  expect(auth).toHaveAttribute(
    "aria-controls",
    screen.getByRole("tabpanel", { name: "Authentication" }).id,
  );
  expect(screen.getAllByRole("tab", { selected: true })).toHaveLength(1);
  await screen.findByRole("region", { name: "Authentication" });
  fireEvent.keyDown(auth, { key: "ArrowRight" });
  await screen.findByRole("region", { name: "Provider management" });
  expect(
    screen.queryByRole("region", { name: "Authentication" }),
  ).not.toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole("tab", { name: "Data sources" }), {
    key: "End",
  });
  await screen.findByRole("region", { name: "Diagnostics" });
  expect(
    screen.getByRole("tab", { name: "Connection diagnostics" }),
  ).toHaveFocus();
  expect(change.mock.calls.map(([tab]) => tab)).toEqual([
    "auth",
    "providers",
    "diagnostics",
  ]);
  expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
});

it("follows the tab supplied by route state and labels its active panel", async () => {
  const view = render(<SettingsPage setup={{ tab: "network" }} />);
  const network = screen.getByRole("tab", { name: "Network inspector" });
  expect(network).toHaveAttribute("aria-selected", "true");
  await screen.findByRole("region", { name: "Network records" });

  view.rerender(<SettingsPage setup={{ tab: "diagnostics" }} />);
  const diagnostics = screen.getByRole("tab", {
    name: "Connection diagnostics",
  });
  expect(diagnostics).toHaveAttribute("aria-selected", "true");
  expect(network).toHaveAttribute("aria-selected", "false");
  const panel = screen.getByRole("tabpanel", {
    name: "Connection diagnostics",
  });
  expect(diagnostics).toHaveAttribute("aria-controls", panel.id);
  expect(panel).toHaveAttribute("aria-labelledby", diagnostics.id);
  await screen.findByRole("region", { name: "Diagnostics" });
  expect(
    screen.queryByRole("region", { name: "Network records" }),
  ).not.toBeInTheDocument();
});

it("translates media settings independently of the library page bundle", async () => {
  await i18n.changeLanguage("zh-CN");
  render(<SettingsPage />);
  expect(screen.getByRole("heading", { name: "媒体类型" })).toBeInTheDocument();
  expect(screen.queryByText("kind")).not.toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "网络检查器" })).toBeInTheDocument();
});

it("keeps the durable session and current media when returning from setup", () => {
  const session = "11111111-1111-4111-8111-111111111111";
  render(
    <SettingsPage
      setup={{
        from: "scraper",
        session,
        media: "22222222-2222-4222-8222-222222222222",
      }}
    />,
  );
  const url = new URL(
    screen
      .getByRole("link", { name: "Return to scraper" })
      .getAttribute("href") ?? "",
    "https://example.test",
  );
  expect(url.pathname).toBe("/scraper");
  expect(url.searchParams.get("session")).toBe(session);
  expect(url.searchParams.get("media")).toBe(
    "22222222-2222-4222-8222-222222222222",
  );
  expect(url.searchParams.has("queue")).toBe(false);
});
