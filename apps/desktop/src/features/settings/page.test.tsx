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
    search?: Record<string, string | undefined>;
    children: React.ReactNode;
  }) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).filter(
        (entry): entry is [string, string] => typeof entry[1] === "string",
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
