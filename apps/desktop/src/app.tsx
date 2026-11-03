import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  lazyRouteComponent,
  RouterProvider,
} from "@tanstack/react-router";
import { AppShell } from "./components/shell/app-shell";
import { LibraryPage, validateLibrarySearch } from "./features/library/browse";
import { PrivacyProvider } from "./features/library/context";
import { MediaDetailPage } from "./features/library/detail";
import { validateScannerSearch } from "./features/library/scanner-navigation";
import {
  NetworkInspectorPage,
  ProvidersPage,
} from "./features/providers/pages";
import "./features/scraper/navigation";
import { validateScraperSearch } from "./features/scraper/search";
import "./features/music/translations";
import "./features/tools/navigation";
import { validateToolsSearch } from "./features/tools/search";

const DashboardPage = lazyRouteComponent(
  () => import("./features/library/pages"),
  "DashboardPage",
);
const RootsPage = lazyRouteComponent(
  () => import("./features/library/pages"),
  "RootsPage",
);
const SettingsPage = lazyRouteComponent(
  () => import("./features/library/pages"),
  "SettingsPage",
);
const ScannerPage = lazyRouteComponent(
  () => import("./features/library/scanner"),
  "ScannerPage",
);
const TasksPage = lazyRouteComponent(
  () => import("./features/library/scanner"),
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
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  validateSearch: validateLibrarySearch,
  component: () => {
    const search = libraryRoute.useSearch();
    const navigate = libraryRoute.useNavigate();
    return (
      <LibraryPage
        search={search}
        onSearch={(patch, replace) => void navigate({ search: patch, replace })}
      />
    );
  },
});
const adultRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult",
  component: lazyRouteComponent(
    () => import("./features/adult/page"),
    "AdultPage",
  ),
});
const adultToolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult/tools",
  component: lazyRouteComponent(
    () => import("./features/adult/page"),
    "AdultToolsPage",
  ),
});
const MediaToolsPage = lazyRouteComponent(
  () => import("./features/tools/page"),
  "MediaToolsPage",
);
const toolsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tools",
  validateSearch: validateToolsSearch,
  component: () => {
    const { media } = toolsRoute.useSearch();
    return <MediaToolsPage key={media ?? "local"} mediaId={media} />;
  },
});
const adultDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/adult/media/$mediaId",
  component: () => {
    const { mediaId } = adultDetailRoute.useParams();
    return <AdultDetailRoute mediaId={mediaId} />;
  },
});
const AdultDetailRoute = lazyRouteComponent(
  () => import("./features/adult/page"),
  "AdultMediaPage",
);
const detailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/media/$mediaId",
  component: () => {
    const { mediaId } = detailRoute.useParams();
    return <MediaDetailPage mediaId={mediaId} />;
  },
});
const rootsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/roots",
  component: () => {
    const navigate = rootsRoute.useNavigate();
    return (
      <RootsPage
        onNavigateScan={(search) => void navigate({ to: "/scanner", search })}
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
        key={search.task ?? "no-task"}
        search={search}
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
    const { media } = scraperRoute.useSearch();
    return <ScraperPage key={media ?? "search"} mediaId={media} />;
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
  component: TasksPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});
const diagnosticsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/diagnostics",
  component: lazyRouteComponent(
    () => import("./features/maintenance/page"),
    "MaintenancePage",
  ),
});
const providersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/providers",
  component: ProvidersPage,
});
const networkRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/network",
  component: NetworkInspectorPage,
});
const router = createRouter({
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
    providersRoute,
    networkRoute,
    settingsRoute,
    diagnosticsRoute,
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
