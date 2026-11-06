import { createMemoryHistory, createRootRoute, createRoute, createRouter, Outlet, RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import i18n from "../../i18n";
import { validateScraperSearch } from "../scraper/search";
import { validateToolsRouteSearch } from "./search";
import { ToolsSectionNavigation, ToolsSidebarNavigation } from "./section-navigation";

beforeEach(async () => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  await i18n.changeLanguage("en-US");
});

async function show(entry: string, inline = false) {
  const root = createRootRoute({ component: () => <><ul><ToolsSidebarNavigation /></ul>{inline && <ToolsSectionNavigation current="local" />}<Outlet /></> });
  const tools = createRoute({ getParentRoute: () => root, path: "/tools", validateSearch: validateToolsRouteSearch, component: () => <h1>Tools</h1> });
  const scraper = createRoute({ getParentRoute: () => root, path: "/scraper", validateSearch: validateScraperSearch, component: () => <h1>Scraper</h1> });
  const library = createRoute({ getParentRoute: () => root, path: "/library", component: () => <h1>Library</h1> });
  const router = createRouter({ routeTree: root.addChildren([tools, scraper, library]), history: createMemoryHistory({ initialEntries: [entry] }), defaultPendingMs: 0, defaultPendingMinMs: 0 });
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

it.each([
  ["/tools?view=health&q=Atlas&root=folder&sort=title_asc", "Checks and monitoring"],
  ["/scraper?session=11111111-1111-4111-8111-111111111111&media=film", "Metadata lookup"],
])("collapses only child navigation without changing the URL or selection on %s", async (entry, current) => {
  const router = await show(entry);
  const location = router.state.location.href;
  const collapse = screen.getByRole("button", { name: "Collapse media tools" });
  expect(collapse).toHaveAttribute("aria-expanded", "true");
  const childrenId = collapse.getAttribute("aria-controls")!;
  expect(document.getElementById(childrenId)).toContainElement(screen.getByRole("navigation", { name: "Media tools navigation" }));
  fireEvent.click(collapse);
  expect(screen.queryByRole("navigation", { name: "Media tools navigation" })).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Media tools" })).toHaveAttribute("data-section-active", "true");
  expect(router.state.location.href).toBe(location);
  const expand = screen.getByRole("button", { name: "Expand media tools" });
  expect(expand).toHaveAttribute("aria-expanded", "false");
  expect(expand).toHaveAttribute("aria-controls", childrenId);
  fireEvent.click(expand);
  expect(screen.getByRole("link", { name: current })).toHaveAttribute("aria-current", "page");
  expect(router.state.location.href).toBe(location);
});

it("offers expanded child destinations before entering the tools section without marking one current", async () => {
  const router = await show("/library");
  const navigation = screen.getByRole("navigation", { name: "Media tools navigation" });
  expect(within(navigation).getAllByRole("link")).toHaveLength(3);
  expect(within(navigation).queryAllByRole("link", { current: "page" })).toHaveLength(0);
  expect(screen.getByRole("link", { name: "Media tools" })).not.toHaveAttribute("data-section-active");
  fireEvent.click(within(navigation).getByRole("link", { name: "Checks and monitoring" }), { button: 0 });
  await waitFor(() => expect(router.state.location.search).toEqual({ view: "health" }));
  expect(router.state.location.pathname).toBe("/tools");
});

it("retains accessible icon names when compact sidebar labels are hidden", async () => {
  await show("/tools?view=health");
  for (const label of document.querySelectorAll<HTMLElement>(".app-nav-label")) label.hidden = true;
  for (const name of ["Media tools", "Local media", "Metadata lookup", "Checks and monitoring"])
    expect(screen.getByRole("link", { name })).toHaveAccessibleName(name);
  fireEvent.click(screen.getByRole("button", { name: "Collapse media tools" }));
  fireEvent.click(screen.getByRole("button", { name: "Expand media tools" }));
  expect(screen.getByRole("link", { name: "Checks and monitoring" })).toHaveAttribute("aria-current", "page");
});

it("keeps the inline tools navigation available when sidebar children are collapsed", async () => {
  await show("/tools", true);
  expect(screen.getAllByRole("navigation", { name: "Media tools navigation" })).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "Collapse media tools" }));
  const inline = screen.getByRole("navigation", { name: "Media tools navigation" });
  expect(within(inline).getByRole("link", { name: "Local media" })).toHaveAttribute("aria-current", "page");
  expect(within(inline).getAllByRole("link")).toHaveLength(3);
});
