import { createMemoryHistory, createRootRoute, createRoute, createRouter, Outlet, RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import i18n from "../../i18n";
import { validateScraperSearch } from "../scraper/search";
import { validateToolsRouteSearch } from "./search";
import { ToolsSidebarNavigation } from "./section-navigation";

beforeEach(async () => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  await i18n.changeLanguage("en-US");
});

async function show(entry: string) {
  const root = createRootRoute({ component: () => <><ul><ToolsSidebarNavigation /></ul><Outlet /></> });
  const tools = createRoute({ getParentRoute: () => root, path: "/tools", validateSearch: validateToolsRouteSearch, component: () => <h1>Tools</h1> });
  const scraper = createRoute({ getParentRoute: () => root, path: "/scraper", validateSearch: validateScraperSearch, component: () => <h1>Scraper</h1> });
  const router = createRouter({ routeTree: root.addChildren([tools, scraper]), history: createMemoryHistory({ initialEntries: [entry] }), defaultPendingMs: 0, defaultPendingMinMs: 0 });
  await router.load();
  render(<RouterProvider router={router} />);
  await screen.findByRole("link", { name: "Media tools" });
  return router;
}

it("restores the health section from URL and retains list filters when returning to local media", async () => {
  const router = await show("/tools?view=health&q=Atlas&root=folder&sort=title_asc");
  expect(within(screen.getByRole("navigation", { name: "Media tools navigation" })).getAllByRole("link", { current: "page" })).toHaveLength(1);
  expect(screen.getByRole("link", { name: "Checks and monitoring" })).toHaveAttribute("aria-current", "page");
  fireEvent.click(screen.getByRole("link", { name: "Local media" }), { button: 0 });
  await waitFor(() => expect(router.state.location.search).toEqual({ q: "Atlas", root: "folder", sort: "title_asc" }));
  expect(screen.getByRole("link", { name: "Local media" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("link", { name: "Media tools" })).toHaveAttribute("data-section-active", "true");
});

it("keeps scraper navigation current without dropping the active review session", async () => {
  const session = "11111111-1111-4111-8111-111111111111";
  const router = await show(`/scraper?session=${session}&media=film`);
  const link = screen.getByRole("link", { name: "Metadata lookup" });
  expect(link).toHaveAttribute("aria-current", "page");
  expect(within(screen.getByRole("navigation", { name: "Media tools navigation" })).getAllByRole("link", { current: "page" })).toHaveLength(1);
  fireEvent.click(link, { button: 0 });
  await waitFor(() => expect(router.state.location.search).toEqual({ session, media: "film" }));
});

it("does not expose ordinary scraper or health links in the isolated adult lane", async () => {
  await show("/tools?kind=adult_video&media=private");
  expect(screen.getByRole("link", { name: "Local media" })).toHaveAttribute("aria-current", "page");
  expect(screen.queryByRole("link", { name: "Metadata lookup" })).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "Checks and monitoring" })).not.toBeInTheDocument();
});
