import { Link, Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import {
  FolderOpen,
  Layers3,
  ListTodo,
  Moon,
  Search,
  Settings2,
  Sun,
  Wrench,
} from "lucide-react";
import { type FormEvent, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { setTheme, useTheme } from "../../lib/theme";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Toaster } from "../ui/sonner";
import { LibraryNavigation, navItemClassName } from "./library-navigation";
import { useTaskEvents } from "./use-task-events";
import { useWindowChrome, type WindowChrome } from "./use-window-controls";
import { WindowControls } from "./window-controls";

const toolbarSearchClassName =
  "relative flex min-w-0 w-[min(100%,480px)] items-center [&_[data-slot=input]]:h-[42px] [&_[data-slot=input]]:pl-[43px] [&_[data-slot=input]]:pr-11 [&_[data-slot=input]]:rounded-[24px] [&_[data-slot=input]]:bg-muted [&_[data-slot=input]]:backdrop-blur-[20px] [&>[data-slot=button]]:absolute [&>[data-slot=button]]:right-0.5 [&>[data-slot=button]]:rounded-full [&_label]:w-full";

const workEntries = [
  ["/roots", "roots", FolderOpen],
  ["/tools", "videoTools", Wrench],
  ["/tasks", "tasks", ListTodo],
] as const;

function Sidebar({ chrome }: { chrome: WindowChrome }) {
  const { t } = useTranslation();
  return (
    <aside className="app-sidebar fixed inset-y-0 left-0 z-40 flex w-[var(--sidebar-width)] flex-col overflow-hidden overscroll-none border-r border-border bg-sidebar">
      <div
        className="app-brand flex h-[88px] shrink-0 items-center gap-3 px-[23px] [@media(max-width:1050px)]:justify-center [@media(max-width:1050px)]:p-0 [@media(max-width:600px)]:h-[76px] group-data-[window-platform=macos]/shell:h-[112px] group-data-[window-platform=macos]/shell:pt-8"
        data-tauri-drag-region={chrome.platform ? "deep" : undefined}
      >
        <span className="grid size-9 shrink-0 place-items-center rounded-[11px] bg-primary text-primary-foreground [@media(max-width:600px)]:size-8">
          <Layers3 size={23} aria-hidden="true" />
        </span>
        <span className="text-[21px] font-[650] tracking-[-0.6px] [@media(max-width:1050px)]:hidden">
          MediaAtlas
        </span>
      </div>
      <nav
        className="app-nav min-h-0 flex-1 overflow-y-auto overscroll-y-contain px-[13px] pt-2 pb-5 [@media(600px<width<=1050px)]:px-[11px] [@media(max-width:600px)]:px-1.5"
        aria-label={t("local")}
      >
        <ul className="m-0 flex list-none flex-col gap-1 p-0 [&>li]:min-w-0">
          <LibraryNavigation />
          <li
            className="mx-3 my-[15px] h-px bg-border [@media(max-width:1050px)]:mx-2"
            aria-hidden="true"
          />
          {workEntries.map(([to, label, Icon]) => (
            <li key={to}>
              <Button asChild variant="ghost" className={navItemClassName}>
                <Link
                  to={to}
                  activeOptions={{ exact: true, includeSearch: false }}
                  activeProps={{ className: "active" }}
                  aria-label={t(label)}
                >
                  <Icon size={20} aria-hidden="true" />
                  <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">
                    {t(label)}
                  </span>
                </Link>
              </Button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="app-sidebar-bottom mt-auto shrink-0 border-t border-border px-[13px] pt-4 pb-[22px] [@media(600px<width<=1050px)]:px-[11px] [@media(max-width:600px)]:px-1.5 [@media(max-width:600px)]:py-3">
        <Button asChild variant="ghost" className={navItemClassName}>
          <Link
            to="/settings"
            activeOptions={{ includeSearch: false }}
            activeProps={{ className: "active" }}
            aria-label={t("settings")}
          >
            <Settings2 size={20} aria-hidden="true" />
            <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">
              {t("settings")}
            </span>
          </Link>
        </Button>
      </div>
    </aside>
  );
}

function Toolbar({ chrome }: { chrome: WindowChrome }) {
  const { t } = useTranslation();
  const theme = useTheme();
  const pathname = useLocation({ select: (location) => location.pathname });
  const navigate = useNavigate();
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = new FormData(event.currentTarget).get("q");
    void navigate({
      to: "/library",
      search: {
        q: typeof query === "string" ? query.trim() || undefined : undefined,
      },
    });
  }
  return (
    <header
      className="app-toolbar fixed inset-x-0 top-0 left-[var(--sidebar-width)] z-30 flex h-[var(--toolbar-height)] items-center gap-6 px-10 py-0 bg-[linear-gradient(color-mix(in_srgb,var(--color-background)_93%,transparent),color-mix(in_srgb,var(--color-background)_80%,transparent))] backdrop-blur-[16px] [@media(600px<width<=1050px)]:px-6 [@media(max-width:600px)]:gap-1.5 [@media(max-width:600px)]:px-2 [@media(width>600px)]:group-data-[window-platform=windows]/shell:pr-[168px] [@media(max-width:600px)]:group-data-[window-platform=windows]/shell:px-4 [@media(max-width:600px)]:group-data-[window-platform=windows]/shell:pt-9"
      data-tauri-drag-region={chrome.platform ? "deep" : undefined}
    >
      {pathname === "/library" ? (
        <div
          id="library-toolbar-search"
          className={toolbarSearchClassName}
          data-tauri-drag-region="false"
        />
      ) : (
        <form
          key={pathname}
          role="search"
          aria-label={t("search")}
          className={toolbarSearchClassName}
          onSubmit={submit}
          data-tauri-drag-region="false"
        >
          <Search
            className="pointer-events-none absolute left-4 z-1 text-muted-foreground"
            size={19}
            aria-hidden="true"
          />
          <Input
            type="search"
            name="q"
            aria-label={t("search")}
            placeholder={t("shellSearchPlaceholder")}
          />
          <Button
            type="submit"
            variant="ghost"
            size="icon"
            aria-label={t("search")}
          >
            <Search size={17} aria-hidden="true" />
          </Button>
        </form>
      )}
      <div
        className="app-toolbar-actions ml-auto flex items-center gap-2 text-muted-foreground [@media(max-width:600px)]:gap-0"
        data-tauri-drag-region="false"
      >
        <Button
          variant="ghost"
          size="icon"
          aria-label={t(theme === "dark" ? "switchToLight" : "switchToDark")}
          onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
        >
          {theme === "dark" ? (
            <Sun size={19} aria-hidden="true" />
          ) : (
            <Moon size={19} aria-hidden="true" />
          )}
        </Button>
      </div>
      <WindowControls chrome={chrome} />
    </header>
  );
}

export function AppShell() {
  const { i18n } = useTranslation();
  const chrome = useWindowChrome();
  useTaskEvents();
  useEffect(() => {
    document.documentElement.lang = i18n.resolvedLanguage ?? "zh-CN";
  }, [i18n.resolvedLanguage]);
  return (
    <div
      className="app-shell group/shell h-dvh overflow-hidden overscroll-none [--sidebar-width:228px] [--toolbar-height:76px] [@media(600px<width<=1050px)]:[--sidebar-width:76px] [@media(max-width:600px)]:[--sidebar-width:60px] [@media(max-width:600px)]:data-[window-platform=macos]:[--sidebar-width:76px] [@media(max-width:600px)]:data-[window-platform=windows]:[--toolbar-height:112px]"
      data-slot="app-shell"
      data-window-platform={chrome.platform ?? undefined}
    >
      <Sidebar chrome={chrome} />
      <div
        className="app-main ml-[var(--sidebar-width)] h-full min-w-0 overflow-x-hidden overflow-y-auto overscroll-y-contain scroll-pt-[calc(var(--toolbar-height)_+_12px)] [scrollbar-gutter:stable]"
        id="app-main-scroll"
      >
        <Toolbar chrome={chrome} />
        <main className="min-w-0 px-10 pt-[104px] pb-10 [@media(600px<width<=1050px)]:px-6 [@media(600px<width<=1050px)]:pt-[100px] [@media(600px<width<=1050px)]:pb-7 [@media(max-width:600px)]:px-4 [@media(max-width:600px)]:pt-[94px] [@media(max-width:600px)]:pb-6 [@media(max-width:600px)]:group-data-[window-platform=windows]/shell:pt-[calc(var(--toolbar-height)_+_18px)]">
          <Outlet />
        </main>
      </div>
      <Toaster />
    </div>
  );
}
