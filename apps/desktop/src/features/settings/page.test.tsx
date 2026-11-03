import { render, screen } from "@testing-library/react";
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
