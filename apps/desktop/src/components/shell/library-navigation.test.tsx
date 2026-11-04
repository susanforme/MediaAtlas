import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { validateLibrarySearch } from "../../features/library/browse-model";
import "../../features/music/navigation";
import i18n from "../../i18n";
import { LibraryNavigation } from "./library-navigation";

const privacy = vi.hoisted(() => ({ includeAdult: false }));
vi.mock("../../features/library/context", () => ({
  usePrivacy: () => privacy,
}));

beforeEach(async () => {
  privacy.includeAdult = false;
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  await i18n.changeLanguage("en-US");
});

async function renderNavigation(entry: string) {
  const root = createRootRoute({
    component: () => (
      <>
        <nav aria-label="Library navigation">
          <ul>
            <LibraryNavigation />
          </ul>
        </nav>
        <Outlet />
      </>
    ),
  });
  const library = createRoute({
    getParentRoute: () => root,
    path: "/library",
    validateSearch: validateLibrarySearch,
    component: () => <h1>Library contents</h1>,
  });
  const music = createRoute({
    getParentRoute: () => root,
    path: "/music",
    component: () => <h1>Music contents</h1>,
  });
  const artist = createRoute({
    getParentRoute: () => root,
    path: "/music/artists/$id",
    component: () => <h1>Artist details</h1>,
  });
  const media = createRoute({
    getParentRoute: () => root,
    path: "/media/$id",
    component: () => <h1>Media details</h1>,
  });
  const history = createMemoryHistory({ initialEntries: [entry] });
  const router = createRouter({
    routeTree: root.addChildren([library, music, artist, media]),
    history,
    defaultPendingMs: 0,
    defaultPendingMinMs: 0,
  });
  await router.load();
  const view = render(<RouterProvider router={router} />);
  await screen.findByRole("navigation", { name: "Library navigation" });
  return { router, ...view };
}

function link(name: string) {
  return screen.getByRole("link", { name });
}

it("keeps the parent as a real all-library link that clears the selected category", async () => {
  const { router } = await renderNavigation("/library?kind=movie&q=Arrival");
  expect(link("Media library")).toHaveAttribute("href", "/library");
  expect(link("Media library")).not.toHaveAttribute("aria-current");
  expect(link("Media library")).toHaveAttribute("data-section-active", "true");
  expect(link("Movie")).toHaveAttribute("aria-current", "page");
  expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
  fireEvent.click(link("Media library"), { button: 0 });
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/library");
    expect(router.state.location.search.kind).toBeUndefined();
    expect(link("Movie")).not.toHaveAttribute("aria-current");
  });
  expect(link("Media library")).toHaveClass("active");
  expect(link("Media library")).toHaveAttribute("aria-current", "page");
  expect(screen.queryByRole("button", { name: "Media library" })).toBeNull();
});

it("keeps the all-library entry current with non-category filters", async () => {
  await renderNavigation("/library?q=night&sort=title_asc&page=2");
  expect(link("Media library")).toHaveClass("active");
  expect(link("Media library")).toHaveAttribute("aria-current", "page");
  expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
});

it.each(["/media/movie-1", "/music/artists/artist-1"])(
  "retains library section context on %s without marking its overview current",
  async (entry) => {
    await renderNavigation(entry);
    expect(link("Media library")).toHaveAttribute("data-section-active", "true");
    expect(link("Media library")).not.toHaveAttribute("aria-current");
    expect(link("Media library")).not.toHaveClass("active");
  },
);

it.each([
  ["Movie", "movie"],
  ["TV series", "tv_series"],
  ["Anime", "anime"],
] as const)(
  "opens %s directly and marks only its category active",
  async (name, kind) => {
    const { router } = await renderNavigation("/library");
    expect(link(name)).not.toHaveAttribute("aria-current");
    fireEvent.click(link(name), { button: 0 });
    await waitFor(() => {
      expect(router.state.location.search.kind).toBe(kind);
      expect(link(name)).toHaveAttribute("aria-current", "page");
    });
    expect(link("Media library")).toHaveAttribute("data-section-active", "true");
    expect(link("Media library")).not.toHaveClass("active");
    expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
    expect(link(name)).toHaveClass("active");
    for (const other of ["Movie", "TV series", "Anime"].filter(
      (item) => item !== name,
    ))
      expect(link(other)).not.toHaveAttribute("aria-current");
  },
);

it.each([
  ["Movie", "movie"],
  ["TV series", "tv_series"],
  ["Anime", "anime"],
] as const)(
  "retains the library section and current %s alongside query, sort, health and pagination",
  async (name, kind) => {
    const { router } = await renderNavigation(
      `/library?kind=${kind}&q=night&sort=title_asc&health=missing_poster&page=2`,
    );
    expect(router.state.location.search).toMatchObject({
      kind,
      q: "night",
      sort: "title_asc",
      health: "missing_poster",
      page: 2,
    });
    expect(link("Media library")).toHaveAttribute("data-section-active", "true");
    expect(link("Media library")).not.toHaveAttribute("aria-current");
    expect(link(name)).toHaveClass("active");
    expect(link(name)).toHaveAttribute("aria-current", "page");
    expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
  },
);

it("hides the adult category until the privacy context permits it", async () => {
  await renderNavigation("/library?kind=adult_video");
  expect(screen.queryByRole("link", { name: "Adult video" })).toBeNull();
  expect(screen.getAllByRole("link")).toHaveLength(5);
});

it("uses the same category navigation and active state for opted-in adult media", async () => {
  privacy.includeAdult = true;
  const { router } = await renderNavigation("/library");
  fireEvent.click(link("Adult video"), { button: 0 });
  await waitFor(() => {
    expect(router.state.location.search.kind).toBe("adult_video");
    expect(link("Adult video")).toHaveClass("active");
  });
  expect(link("Media library")).toHaveAttribute("data-section-active", "true");
  expect(link("Media library")).not.toHaveAttribute("aria-current");
  expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
});

it("keeps compact icon links named and sends music to its actual workspace", async () => {
  const { router, container } = await renderNavigation("/library?kind=anime");
  for (const label of container.querySelectorAll<HTMLElement>(".app-nav-label"))
    label.hidden = true;
  for (const name of ["Media library", "Movie", "TV series", "Anime", "Music"])
    expect(link(name)).toHaveAccessibleName(name);
  fireEvent.click(link("Music"), { button: 0 });
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/music");
    expect(link("Music")).toHaveAttribute("aria-current", "page");
  });
  expect(link("Media library")).not.toHaveAttribute("aria-current");
  expect(link("Media library")).toHaveAttribute("data-section-active", "true");
  expect(link("Media library")).not.toHaveClass("active");
  expect(link("Anime")).not.toHaveAttribute("aria-current");
  expect(screen.getAllByRole("link", { current: "page" })).toHaveLength(1);
});
