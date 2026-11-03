import {
  QueryClient,
  QueryClientProvider,
  useQueryClient,
} from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Layers3 } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { LibraryPage, validateLibrarySearch } from "./features/library/browse";
import { PrivacyProvider, usePrivacy } from "./features/library/context";
import { MediaDetailPage } from "./features/library/detail";
import {
  DashboardPage,
  RootsPage,
  SettingsPage,
} from "./features/library/pages";
import {
  ScannerPage,
  TasksPage,
  validateScannerSearch,
} from "./features/library/scanner";
import { HomePage } from "./routes/home";

function useTaskEvents() {
  const client = useQueryClient();
  useEffect(() => {
    if (!isTauri()) return;
    let closed = false;
    const subscriptions: UnlistenFn[] = [];
    for (const event of [
      "task-created",
      "task-updated",
      "task-needs-review",
      "task-completed",
    ]) {
      void listen<{ schema_version: number }>(event, (message) => {
        if (message.payload.schema_version !== 1) return;
        for (const key of [
          "tasks",
          "preview",
          "summary",
          "media",
          "facets",
          "roots",
        ])
          void client.invalidateQueries({ queryKey: [key] });
      })
        .then((unlisten) => {
          if (closed) unlisten();
          else subscriptions.push(unlisten);
        })
        .catch(() => {
          /* Database polling remains authoritative. */
        });
    }
    return () => {
      closed = true;
      for (const unlisten of subscriptions) unlisten();
    };
  }, [client]);
}
function Layout() {
  const { t, i18n } = useTranslation();
  const { includeAdult } = usePrivacy();
  useTaskEvents();
  useEffect(() => {
    document.documentElement.lang = i18n.resolvedLanguage ?? "zh-CN";
  }, [i18n.resolvedLanguage]);
  return (
    <div className="min-h-screen">
      <header className="app-header">
        <div className="flex items-center gap-3">
          <span className="flex size-9 items-center justify-center rounded-xl bg-primary text-white">
            <Layers3 className="size-5" aria-hidden="true" />
          </span>
          <span className="font-semibold tracking-tight">MediaAtlas</span>
          <span className="hidden text-xs text-muted-foreground md:block">
            {t("localOnly")}
          </span>
        </div>
        <select
          aria-label={t("language")}
          value={i18n.resolvedLanguage ?? "zh-CN"}
          onChange={(event) => void i18n.changeLanguage(event.target.value)}
        >
          <option value="zh-CN">简体中文</option>
          <option value="en-US">English (US)</option>
        </select>
      </header>
      <div className="app-body">
        <nav className="app-nav" aria-label={t("local")}>
          {(
            [
              ["/", "dashboard"],
              ["/library", "library"],
              ["/roots", "roots"],
              ["/scanner", "scanner"],
              ["/tasks", "tasks"],
              ["/settings", "settings"],
              ["/diagnostics", "diagnostics"],
            ] as const
          ).map(([to, label]) => (
            <Link
              key={to}
              to={to}
              activeOptions={{ exact: true }}
              activeProps={{ className: "active" }}
            >
              {t(label)}
            </Link>
          ))}
          {includeAdult && (
            <Link to="/adult" activeProps={{ className: "active" }}>
              {t("adultLibrary")}
            </Link>
          )}
        </nav>
        <main className="app-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
function RootLayout() {
  return (
    <PrivacyProvider>
      <Layout />
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
  validateSearch: validateLibrarySearch,
  component: () => {
    const search = adultRoute.useSearch();
    const navigate = adultRoute.useNavigate();
    return (
      <LibraryPage
        adult
        search={search}
        onSearch={(patch, replace) => void navigate({ search: patch, replace })}
      />
    );
  },
});
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
  component: RootsPage,
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
  component: HomePage,
});
const router = createRouter({
  routeTree: rootRoute.addChildren([
    indexRoute,
    libraryRoute,
    adultRoute,
    detailRoute,
    rootsRoute,
    scannerRoute,
    tasksRoute,
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
