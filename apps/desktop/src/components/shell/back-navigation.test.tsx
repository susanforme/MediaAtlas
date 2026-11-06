import { createMemoryHistory, createRootRoute, createRoute, createRouter, Link, Outlet, RouterProvider } from "@tanstack/react-router";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { validateLibrarySearch } from "../../features/library/browse-model";
import "../../features/music/translations";
import i18n from "../../i18n";
import { BackNavigation } from "./back-navigation";

beforeEach(async () => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  await i18n.changeLanguage("en-US");
});

async function workspace(initial: string, adult = false) {
  const root = createRootRoute({
    component: () => <div id="app-main-scroll" ref={(element) => { if (element) element.scrollTo = vi.fn(); }}><Outlet /></div>,
  });
  const library = createRoute({
    getParentRoute: () => root,
    path: "/library",
    validateSearch: validateLibrarySearch,
    component: () => <><h1>Library contents</h1><Link to="/media/$mediaId" params={{ mediaId: "selected" }}>Selected film</Link></>,
  });
  const media = createRoute({
    getParentRoute: () => root,
    path: "/media/$mediaId",
    component: () => <><BackNavigation adult={adult} /><h1>Film details</h1></>,
  });
  const tools = createRoute({
    getParentRoute: () => root,
    path: "/tools",
    component: () => <h1>Media tools</h1>,
  });
  const music = createRoute({
    getParentRoute: () => root,
    path: "/music",
    component: () => <h1>Music library</h1>,
  });
  const album = createRoute({
    getParentRoute: () => root,
    path: "/music/album/$releaseId",
    component: () => <><BackNavigation fallbackTo="/music" /><h1>Album details</h1></>,
  });
  const router = createRouter({
    routeTree: root.addChildren([library, media, tools, music, album]),
    history: createMemoryHistory({ initialEntries: [initial] }),
    scrollRestoration: true,
    scrollToTopSelectors: ["#app-main-scroll"],
  });
  await router.load();
  render(<RouterProvider router={router} />);
  await screen.findByRole("heading");
  return router;
}

it("returns to the actual library history entry with its filters, view and cursor", async () => {
  const router = await workspace("/library?kind=movie&root=films&q=night&sort=title_asc&cursor=second-page&page=2&view=list");
  const previous = router.state.location;
  fireEvent.click(screen.getByRole("link", { name: "Selected film" }), { button: 0 });
  const back = await screen.findByRole("button", { name: "Back" });
  expect(screen.queryByRole("link", { name: "Back to library" })).toBeNull();
  fireEvent.click(back);
  await screen.findByRole("heading", { name: "Library contents" });
  expect(router.state.location.href).toBe(previous.href);
  expect(router.state.location.state.__TSR_key).toBe(previous.state.__TSR_key);
});

it("returns to the actual caller instead of redirecting a tool inspection to the library", async () => {
  const router = await workspace("/tools");
  await act(async () => { await router.navigate({ to: "/media/$mediaId", params: { mediaId: "selected" } }); });
  fireEvent.click(await screen.findByRole("button", { name: "Back" }));
  await screen.findByRole("heading", { name: "Media tools" });
  expect(router.state.location.pathname).toBe("/tools");
});

it.each([false, true])("has a usable library fallback on a directly opened detail (adult=%s)", async (adult) => {
  const router = await workspace("/media/selected", adult);
  expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "Back to library" }), { button: 0 });
  await screen.findByRole("heading", { name: "Library contents" });
  expect(router.state.location.search.kind).toBe(adult ? "adult_video" : undefined);
  expect(router.history.location.state.__TSR_index).toBe(0);
});

it("returns music details to music when opened without an application history", async () => {
  const router = await workspace("/music/album/selected");
  fireEvent.click(screen.getByRole("link", { name: i18n.t("musicBack") }), { button: 0 });
  await waitFor(() => expect(router.state.location.pathname).toBe("/music"));
  expect(router.history.location.state.__TSR_index).toBe(0);
});
