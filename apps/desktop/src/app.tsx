import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  lazyRouteComponent,
  redirect,
  RouterProvider,
} from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { AppShell } from "./components/shell/app-shell";
import { validateLibrarySearch } from "./features/library/browse-model";
import { PrivacyProvider, usePrivacy } from "./features/library/context";
import { validateScannerSearch } from "./features/library/scanner-navigation";
import { Loading } from "./features/library/shared";
import {
  SETUP_KINDS,
  type SetupKind,
  setupReturnLocation,
  validateProviderSetupSearch,
} from "./features/settings/setup-search";
import "./features/scraper/navigation";
import { validateScraperSearch } from "./features/scraper/search";
import "./features/music/navigation";
import "./features/tools/navigation";
import { toolsFilterKey, validateToolsRouteSearch } from "./features/tools/search";
import { cancelRoutePreparations, InitialRouteLoading, prepareDesktopRoute, RouteFailure, type RouteDataContext } from "./lib/route-transition";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, retryOnMount: false, refetchOnWindowFocus: false } },
});

const loadLibrary = () => import("./features/library/browse");
const loadMedia = () => import("./features/library/detail");
const loadAdultLibrary = () => import("./features/adult/library-page");
const loadRoots = () => import("./features/library/pages");
const loadSettings = () => import("./features/settings/page");
const loadScanner = () => import("./features/library/scanner");
const loadTasks = () => import("./features/library/tasks");
const loadTools = () => import("./features/tools/page");
const loadScraper = () => import("./features/scraper/page");
const loadAdultMedia = () => import("./features/adult/page");
const loadMusic = () => import("./features/music/page");

const LibraryPage = lazyRouteComponent(
  loadLibrary,
  "LibraryPage",
);
const MediaDetailPage = lazyRouteComponent(
  loadMedia,
  "MediaDetailPage",
);
const AdultLibraryPage = lazyRouteComponent(
  loadAdultLibrary,
  "AdultLibraryPage",
);
const RootsPage = lazyRouteComponent(
  loadRoots,
  "RootsPage",
);
const SettingsPage = lazyRouteComponent(
  loadSettings,
  "SettingsPage",
);
const ScannerPage = lazyRouteComponent(
  loadScanner,
  "ScannerPage",
);
const TasksPage = lazyRouteComponent(
  loadTasks,
  "TasksPage",
);
function RootLayout() {
  return <AppShell />;
}
const rootRoute = createRootRouteWithContext<RouteDataContext>()({ component: RootLayout });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => { throw redirect({ to: "/library", replace: true }); },
});
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  validateSearch: validateLibrarySearch,
  loaderDeps: ({ search }) => search,
  loader: ({ deps, context, abortController }) => prepareDesktopRoute(
    abortController,
    deps.kind === "adult_video" ? loadAdultLibrary : loadLibrary,
    deps.kind === "adult_video" ? AdultLibraryPage : LibraryPage,
    async (check) => {
      const { prepareLibraryPage } = await import("./features/library/route-data");
      await prepareLibraryPage({ ...context, check }, deps);
    },
  ),
  component: () => {
    const search = libraryRoute.useSearch();
    const navigate = libraryRoute.useNavigate();
    const Page = search.kind === "adult_video" ? AdultLibraryPage : LibraryPage;
    return (
      <Page
        search={search}
        onSearch={(patch, replace) =>
          patch.kind === "music_release"
            ? void navigate({ to: "/music", replace })
            : void navigate({ search: patch, replace })
        }
      />
    );
  },
});
const adultRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult",
  validateSearch: validateLibrarySearch,
  beforeLoad: ({ search }) => { throw redirect({ to: "/library", search: { ...search, kind: "adult_video" }, replace: true }); },
});
const adultToolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult/tools",
  validateSearch: validateToolsRouteSearch,
  beforeLoad: ({ search }) => { throw redirect({ to: "/tools", search: validateToolsRouteSearch({ ...search, kind: "adult_video" }), replace: true }); },
});
const MediaToolsPage = lazyRouteComponent(
  loadTools,
  "MediaToolsPage",
);
const toolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tools",
  validateSearch: validateToolsRouteSearch,
  loaderDeps: ({ search }) => search,
  loader: ({ deps, context, abortController }) => prepareDesktopRoute(abortController, loadTools, MediaToolsPage, async (check) => {
    const { prepareToolsPage } = await import("./features/tools/route-data");
    await prepareToolsPage({ ...context, check }, deps);
  }),
  component: () => {
    const search = toolsRoute.useSearch();
    const navigate = toolsRoute.useNavigate();
    return (
      <MediaToolsPage
        key={toolsFilterKey(search)}
        mediaId={search.media}
        search={search}
        onSearch={(value, replace) => void navigate({ search: value, replace })}
      />
    );
  },
});
const adultDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult/media/$mediaId",
  beforeLoad: ({ params }) => { throw redirect({ to: "/media/$mediaId", params, search: { kind: "adult_video" }, replace: true }); },
});
const AdultDetailRoute = lazyRouteComponent(
  loadAdultMedia,
  "AdultMediaPage",
);
const detailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/media/$mediaId",
  validateSearch: (search: Record<string, unknown>) =>
    search.kind === "adult_video" ? { kind: "adult_video" as const } : {},
  loaderDeps: ({ search }) => search,
  loader: ({ deps, params, context, abortController }) => prepareDesktopRoute(
    abortController,
    deps.kind === "adult_video" ? loadAdultMedia : loadMedia,
    deps.kind === "adult_video" ? AdultDetailRoute : MediaDetailPage,
    async (check) => {
      const { prepareMediaPage } = await import("./features/library/route-data");
      await prepareMediaPage({ ...context, check }, params.mediaId, deps.kind === "adult_video");
    },
  ),
  component: () => {
    const { mediaId } = detailRoute.useParams();
    const search = detailRoute.useSearch();
    return search.kind === "adult_video" ? (
      <AdultDetailRoute mediaId={mediaId} />
    ) : (
      <MediaDetailPage mediaId={mediaId} />
    );
  },
});
const rootsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/roots",
  loader: ({ context, abortController }) => prepareDesktopRoute(abortController, loadRoots, RootsPage, async (check) => {
    const { prepareDirectoriesPage } = await import("./features/library/route-data");
    await prepareDirectoriesPage({ ...context, check });
  }),
  validateSearch: (search: Record<string, unknown>) => ({
    ...(SETUP_KINDS.includes(search.kind as SetupKind)
      ? { kind: search.kind as SetupKind }
      : {}),
    ...(search.add === true || search.add === "true" ? { add: true } : {}),
  }),
  component: () => {
    const navigate = rootsRoute.useNavigate();
    const search = rootsRoute.useSearch();
    return (
      <RootsPage
        initialKind={search.kind}
        initialAdd={search.add}
        adult={search.kind === "adult_video"}
        onNavigateScan={(search) =>
          void navigate({
            to: "/library",
            search: { root: search.root, kind: search.kind },
          })
        }
      />
    );
  },
});
const scannerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/scanner",
  validateSearch: validateScannerSearch,
  loaderDeps: ({ search }) => search,
  loader: ({ context, abortController }) => prepareDesktopRoute(abortController, loadScanner, ScannerPage, async (check) => {
    const { prepareScannerPage } = await import("./features/library/route-data");
    await prepareScannerPage({ ...context, check });
  }),
  component: () => {
    const search = scannerRoute.useSearch();
    const navigate = scannerRoute.useNavigate();
    return (
      <ScannerPage
        key={`${search.kind ?? "ordinary"}:${search.task ?? "no-task"}`}
        search={search}
        onAddRoot={() =>
          void navigate({
            to: "/roots",
            search: search.kind ? { kind: search.kind } : {},
          })
        }
        onBrowse={() =>
          void navigate({
            to: "/library",
            search: { kind: search.kind, root: search.root },
          })
        }
        onSearch={(value) => void navigate({ search: value })}
      />
    );
  },
});
const ScraperPage = lazyRouteComponent(
  loadScraper,
  "ScraperPage",
);
const scraperRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/scraper",
  validateSearch: validateScraperSearch,
  loaderDeps: ({ search }) => search,
  loader: ({ deps, context, abortController }) => prepareDesktopRoute(abortController, loadScraper, ScraperPage, async (check) => {
    const { prepareScraperPage } = await import("./features/scraper/route-data");
    await prepareScraperPage({ ...context, check }, deps);
  }),
  component: () => {
    const { media, queue, session } = scraperRoute.useSearch();
    const navigate = scraperRoute.useNavigate();
    return (
      <ScraperPage
        key={`${session ?? "single"}:${media ?? "search"}`}
        mediaId={media}
        sessionId={session}
        onSessionNavigate={(sessionId, nextMedia) =>
          void navigate({ search: { session: sessionId, media: nextMedia } })
        }
        queue={queue}
        onQueueNavigate={(nextMedia) =>
          void navigate({ search: { media: nextMedia, queue, session } })
        }
      />
    );
  },
});
const MusicAlbumRoute = lazyRouteComponent(
  loadMusic,
  "MusicAlbumPage",
);
const MusicArtistRoute = lazyRouteComponent(
  loadMusic,
  "MusicArtistPage",
);
const MusicPage = lazyRouteComponent(loadMusic, "MusicPage");
const MusicToolsPage = lazyRouteComponent(loadMusic, "MusicToolsPage");
const musicRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music",
  component: MusicPage,
  loader: ({ context, abortController }) => prepareDesktopRoute(abortController, loadMusic, MusicPage, async (check) => {
    const { prepareMusicPage } = await import("./features/music/route-data");
    await prepareMusicPage({ ...context, check });
  }),
});
const musicToolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/tools",
  component: MusicToolsPage,
  loader: ({ context, abortController }) => prepareDesktopRoute(abortController, loadMusic, MusicToolsPage, async (check) => {
    const { prepareMusicPage } = await import("./features/music/route-data");
    await prepareMusicPage({ ...context, check }, true);
  }),
});
const musicAlbumRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/album/$releaseId",
  loader: ({ params, context, abortController }) => prepareDesktopRoute(abortController, loadMusic, MusicAlbumRoute, async (check) => {
    const { prepareAlbumPage } = await import("./features/music/route-data");
    await prepareAlbumPage({ ...context, check }, params.releaseId);
  }),
  component: () => {
    const { releaseId } = musicAlbumRoute.useParams();
    return <MusicAlbumRoute key={releaseId} releaseId={releaseId} />;
  },
});
const musicArtistRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/artist/$artistId",
  loader: ({ params, context, abortController }) => prepareDesktopRoute(abortController, loadMusic, MusicArtistRoute, async (check) => {
    const { prepareArtistPage } = await import("./features/music/route-data");
    await prepareArtistPage({ ...context, check }, params.artistId);
  }),
  component: () => {
    const { artistId } = musicArtistRoute.useParams();
    return <MusicArtistRoute key={artistId} artistId={artistId} />;
  },
});
const tasksRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tasks",
  loader: ({ context, abortController }) => prepareDesktopRoute(abortController, loadTasks, TasksPage, async (check) => {
    const { prepareTasksPage } = await import("./features/library/route-data");
    await prepareTasksPage({ ...context, check });
  }),
  validateSearch: (search: Record<string, unknown>) =>
    search.kind === "adult_video" ? { kind: "adult_video" as const } : {},
  component: TasksPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  validateSearch: validateProviderSetupSearch,
  loaderDeps: ({ search }) => search,
  loader: ({ deps, context, abortController }) => prepareDesktopRoute(abortController, loadSettings, SettingsPage, async (check) => {
    const { prepareSettingsPage } = await import("./features/settings/route-data");
    await prepareSettingsPage({ ...context, check }, deps);
  }),
  component: () => {
    const search = settingsRoute.useSearch();
    const navigate = settingsRoute.useNavigate();
    return (
      <SettingsPage
        setup={search}
        onTabChange={(tab) =>
          void navigate({ search: { ...search, tab }, replace: true })
        }
        onSetupComplete={(root, _task, kind) =>
          void navigate(setupReturnLocation({ ...search, root, kind }))
        }
        returnToScraper={search.from === "scraper"}
        returnMediaId={search.media}
        returnQueue={search.queue}
      />
    );
  },
});
const router = createRouter({
  scrollRestoration: true,
  getScrollRestorationKey: (location) => location.href,
  scrollToTopSelectors: ["#app-main-scroll"],
  context: { queryClient, readSettings: () => null },
  InnerWrap: InitialRouteLoading,
  defaultErrorComponent: RouteFailure,
  defaultStaleReloadMode: "blocking",
  routeTree: rootRoute.addChildren([
    indexRoute,
    libraryRoute,
    adultRoute,
    adultToolsRoute,
    toolsRoute,
    adultDetailRoute,
    detailRoute,
    rootsRoute,
    scannerRoute,
    tasksRoute,
    scraperRoute,
    musicRoute,
    musicToolsRoute,
    musicAlbumRoute,
    musicArtistRoute,
    settingsRoute,
  ]),
});
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
function ApplicationRouter() {
  const privacy = usePrivacy();
  const [ready, setReady] = useState(false);
  useEffect(() => {
    if (!privacy.loading) setReady(true);
  }, [privacy.loading]);
  if (!ready) return <Loading />;
  return <RouterProvider router={router} context={{ queryClient, readSettings: privacy.readSettings }} />;
}

function revokeNavigationAccess() {
  cancelRoutePreparations();
  void router.invalidate({ sync: true });
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <PrivacyProvider onAccessRevoked={revokeNavigationAccess}>
        <ApplicationRouter />
      </PrivacyProvider>
    </QueryClientProvider>
  );
}
