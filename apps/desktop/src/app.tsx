import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { Layers3 } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { HomePage } from "./routes/home";

function AppLayout() {
  const { t, i18n } = useTranslation();
  useEffect(() => {
    document.documentElement.lang = i18n.language;
  }, [i18n.language]);

  return (
    <div className="min-h-screen">
      <header className="flex h-18 items-center justify-between border-b border-border bg-white px-6 sm:px-10">
        <div className="flex items-center gap-3">
          <span className="flex size-9 items-center justify-center rounded-xl bg-primary text-white">
            <Layers3 className="size-5" aria-hidden="true" />
          </span>
          <span className="font-semibold tracking-tight">MediaAtlas</span>
          <span className="ml-3 hidden border-l border-border pl-5 text-xs text-muted-foreground sm:block">
            {t("local")}
          </span>
        </div>
        <select
          className="rounded-lg border border-border bg-white px-3 py-2 text-sm focus-visible:outline-2 focus-visible:outline-primary"
          aria-label={t("language")}
          value={i18n.language}
          onChange={(event) => void i18n.changeLanguage(event.target.value)}
        >
          <option value="zh-CN">简体中文</option>
          <option value="en">English</option>
        </select>
      </header>
      <Outlet />
    </div>
  );
}

const rootRoute = createRootRoute({ component: AppLayout });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});
const router = createRouter({ routeTree: rootRoute.addChildren([indexRoute]) });
const queryClient = new QueryClient();

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
