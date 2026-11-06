import { QueryClient, QueryClientProvider, queryOptions, useQuery } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, lazyRouteComponent, Link, Outlet, redirect, RouterProvider } from "@tanstack/react-router";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import i18n from "../i18n";
import { cancelRoutePreparations, InitialRouteLoading, prepareDesktopRoute, RouteFailure } from "./route-transition";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

beforeEach(async () => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  await i18n.changeLanguage("en-US");
});
afterEach(cancelRoutePreparations);

async function workspace(initial = "/library", failModule = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, retryOnMount: false } } });
  const data = deferred<string>();
  const module = deferred<{ Target: () => React.ReactNode }>();
  const fetchData = vi.fn(() => data.promise);
  const options = queryOptions({ queryKey: ["transition-target"], queryFn: fetchData });
  function Target() {
    const query = useQuery(options);
    return query.isPending ? <p role="status">Target loading</p> : <h1>{query.data}</h1>;
  }
  const load = vi.fn(() => module.promise);
  if (failModule) load.mockRejectedValueOnce(new Error("private module path"));
  const Page = lazyRouteComponent(load, "Target");
  const root = createRootRoute({ component: () => <><nav><Link to="/tools">Open tools</Link><Link to="/settings">Open settings</Link></nav><Outlet /></> });
  const library = createRoute({ getParentRoute: () => root, path: "/library", component: () => <h1>Current library</h1> });
  const tools = createRoute({
    getParentRoute: () => root,
    path: "/tools",
    component: () => <Page />,
    loader: ({ abortController }) => prepareDesktopRoute(abortController, load, Page, async (check) => {
      await client.ensureQueryData(options);
      check();
    }),
  });
  const settings = createRoute({ getParentRoute: () => root, path: "/settings", component: () => <h1>Ready settings</h1> });
  const alias = createRoute({ getParentRoute: () => root, path: "/adult/tools", beforeLoad: () => { throw redirect({ to: "/tools", replace: true }); } });
  const router = createRouter({
    routeTree: root.addChildren([library, tools, settings, alias]),
    history: createMemoryHistory({ initialEntries: [initial] }),
    InnerWrap: InitialRouteLoading,
    defaultErrorComponent: RouteFailure,
    defaultStaleReloadMode: "blocking",
    defaultPendingMs: 0,
  });
  if (initial === "/library") await router.load();
  render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
  return { client, router, data, module, load, options, fetchData, Target };
}

it("retains the current page until both the lazy component and query data are ready", async () => {
  const page = await workspace();
  fireEvent.click(screen.getByRole("link", { name: "Open tools" }), { button: 0 });
  await waitFor(() => expect(page.options.queryFn).toHaveBeenCalled());
  expect(screen.getByRole("heading", { name: "Current library" })).toBeVisible();
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  await act(async () => page.data.resolve("Ready tools"));
  expect(screen.getByRole("heading", { name: "Current library" })).toBeVisible();
  await act(async () => page.module.resolve({ Target: page.Target }));
  expect(await screen.findByRole("heading", { name: "Ready tools" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "Current library" })).not.toBeInTheDocument();
  expect(screen.queryByText("Target loading")).not.toBeInTheDocument();
});

it("shows loading only on an initial route with no previously presented page", async () => {
  const page = await workspace("/tools");
  expect(screen.getByRole("status")).toBeVisible();
  await act(async () => {
    page.data.resolve("Ready tools");
    page.module.resolve({ Target: page.Target });
  });
  expect(await screen.findByRole("heading", { name: "Ready tools" })).toBeVisible();
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});

it("keeps initial loading through a historical redirect until its target is prepared", async () => {
  const page = await workspace("/adult/tools");
  await waitFor(() => expect(page.fetchData).toHaveBeenCalled());
  expect(screen.getByRole("status")).toBeVisible();
  expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  await act(async () => {
    page.data.resolve("Ready tools");
    page.module.resolve({ Target: page.Target });
  });
  expect(await screen.findByRole("heading", { name: "Ready tools" })).toBeVisible();
  expect(page.router.state.location.pathname).toBe("/tools");
});

it("does not let an older slow navigation replace a newer destination", async () => {
  const page = await workspace();
  fireEvent.click(screen.getByRole("link", { name: "Open tools" }), { button: 0 });
  await waitFor(() => expect(page.load).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("link", { name: "Open settings" }), { button: 0 });
  expect(await screen.findByRole("heading", { name: "Ready settings" })).toBeVisible();
  await act(async () => {
    page.data.resolve("Late tools");
    page.module.resolve({ Target: page.Target });
  });
  expect(screen.getByRole("heading", { name: "Ready settings" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "Late tools" })).not.toBeInTheDocument();
  expect(page.router.state.location.pathname).toBe("/settings");
});

it("retains the current page when revisiting a successful route whose query cache was cleared", async () => {
  const page = await workspace();
  page.data.resolve("First tools");
  page.module.resolve({ Target: page.Target });
  fireEvent.click(screen.getByRole("link", { name: "Open tools" }), { button: 0 });
  await screen.findByRole("heading", { name: "First tools" });
  fireEvent.click(screen.getByRole("link", { name: "Open settings" }), { button: 0 });
  await screen.findByRole("heading", { name: "Ready settings" });
  page.client.clear();
  const next = deferred<string>();
  page.fetchData.mockImplementation(() => next.promise);
  const previousCalls = page.fetchData.mock.calls.length;
  fireEvent.click(screen.getByRole("link", { name: "Open tools" }), { button: 0 });
  await waitFor(() => expect(page.fetchData.mock.calls.length).toBeGreaterThan(previousCalls));
  expect(screen.getByRole("heading", { name: "Ready settings" })).toBeVisible();
  expect(screen.queryByText("Target loading")).not.toBeInTheDocument();
  await act(async () => next.resolve("Refreshed tools"));
  expect(await screen.findByRole("heading", { name: "Refreshed tools" })).toBeVisible();
});

it("allows retrying a failed module load without showing private error details", async () => {
  const page = await workspace("/library", true);
  page.data.resolve("Ready tools");
  fireEvent.click(screen.getByRole("link", { name: "Open tools" }), { button: 0 });
  const retry = await screen.findByRole("button", { name: "Check again" });
  expect(screen.queryByText("private module path")).not.toBeInTheDocument();
  page.module.resolve({ Target: page.Target });
  fireEvent.click(retry);
  expect(await screen.findByRole("heading", { name: "Ready tools" })).toBeVisible();
});

it("aborts preparation synchronously and prevents later phases from starting private reads", async () => {
  const controller = new AbortController();
  const module = deferred<undefined>();
  const gate = deferred<undefined>();
  const readPrivate = vi.fn();
  const pending = prepareDesktopRoute(controller, () => module.promise, {}, async (check) => {
    await gate.promise;
    check();
    readPrivate();
  });
  const rejected = expect(pending).rejects.toMatchObject({ name: "AbortError" });
  cancelRoutePreparations();
  expect(controller.signal.aborted).toBe(true);
  gate.resolve(undefined);
  module.resolve(undefined);
  await rejected;
  expect(readPrivate).not.toHaveBeenCalled();
});

it("prevents a failed module's parallel preparation from starting a later phase", async () => {
  const gate = deferred<undefined>();
  const laterRead = vi.fn();
  const pending = prepareDesktopRoute(new AbortController(), async () => { throw new Error("Module unavailable"); }, {}, async (check) => {
    await gate.promise;
    check();
    laterRead();
  });
  await expect(pending).rejects.toThrow("Module unavailable");
  await act(async () => gate.resolve(undefined));
  expect(laterRead).not.toHaveBeenCalled();
});
