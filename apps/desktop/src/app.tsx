import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  lazyRouteComponent,
  Navigate,
  RouterProvider,
} from "@tanstack/react-router";
import { AppShell } from "./components/shell/app-shell";
import { validateLibrarySearch } from "./features/library/browse-model";
import { PrivacyProvider } from "./features/library/context";
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
import { validateToolsRouteSearch } from "./features/tools/search";

const LibraryPage = lazyRouteComponent(
  () => import("./features/library/browse"),
  "LibraryPage",
);
const MediaDetailPage = lazyRouteComponent(
  () => import("./features/library/detail"),
  "MediaDetailPage",
);
const AdultLibraryPage = lazyRouteComponent(
  () => import("./features/adult/library-page"),
  "AdultLibraryPage",
);
const RootsPage = lazyRouteComponent(
  () => import("./features/library/pages"),
  "RootsPage",
);
const SettingsPage = lazyRouteComponent(
  () => import("./features/settings/page"),
  "SettingsPage",
);
const ScannerPage = lazyRouteComponent(
  () => import("./features/library/scanner"),
  "ScannerPage",
);
const TasksPage = lazyRouteComponent(
  () => import("./features/library/tasks"),
  "TasksPage",
);
function RootLayout() {
  return (
    <PrivacyProvider>
      <AppShell />
    </PrivacyProvider>
  );
}
const rootRoute = createRootRoute({ component: RootLayout });
const nfoRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/nfo",
  component: lazyRouteComponent(() => import("./features/nfo/page"), "NfoPage"),
});
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => <Navigate to="/library" replace />,
});
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  validateSearch: validateLibrarySearch,
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
  component: () => (
    <Navigate to="/library" search={{ kind: "adult_video" }} replace />
  ),
});
const adultToolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult/tools",
  component: () => (
    <Navigate to="/tools" search={{ kind: "adult_video" }} replace />
  ),
});
const MediaToolsPage = lazyRouteComponent(
  () => import("./features/tools/page"),
  "MediaToolsPage",
);
const toolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tools",
  validateSearch: validateToolsRouteSearch,
  component: () => {
    const search = toolsRoute.useSearch();
    const navigate = toolsRoute.useNavigate();
    return (
      <MediaToolsPage
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
  component: () => {
    const { mediaId } = adultDetailRoute.useParams();
    return (
      <Navigate
        to="/media/$mediaId"
        params={{ mediaId }}
        search={{ kind: "adult_video" }}
        replace
      />
    );
  },
});
const AdultDetailRoute = lazyRouteComponent(
  () => import("./features/adult/page"),
  "AdultMediaPage",
);
const detailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/media/$mediaId",
  validateSearch: (search: Record<string, unknown>) =>
    search.kind === "adult_video" ? { kind: "adult_video" as const } : {},
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
  () => import("./features/scraper/page"),
  "ScraperPage",
);
const scraperRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/scraper",
  validateSearch: validateScraperSearch,
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
  () => import("./features/music/page"),
  "MusicAlbumPage",
);
const MusicArtistRoute = lazyRouteComponent(
  () => import("./features/music/page"),
  "MusicArtistPage",
);
const musicRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music",
  component: lazyRouteComponent(
    () => import("./features/music/page"),
    "MusicPage",
  ),
});
const musicToolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/tools",
  component: lazyRouteComponent(
    () => import("./features/music/page"),
    "MusicToolsPage",
  ),
});
const musicAlbumRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/album/$releaseId",
  component: () => {
    const { releaseId } = musicAlbumRoute.useParams();
    return <MusicAlbumRoute key={releaseId} releaseId={releaseId} />;
  },
});
const musicArtistRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/music/artist/$artistId",
  component: () => {
    const { artistId } = musicArtistRoute.useParams();
    return <MusicArtistRoute key={artistId} artistId={artistId} />;
  },
});
const tasksRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tasks",
  validateSearch: (search: Record<string, unknown>) =>
    search.kind === "adult_video" ? { kind: "adult_video" as const } : {},
  component: TasksPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  validateSearch: validateProviderSetupSearch,
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
  defaultPendingComponent: Loading,
  defaultPendingMs: 0,
  defaultPendingMinMs: 0,
  routeTree: rootRoute.addChildren([
    indexRoute,
    nfoRoute,
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
const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
});
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
