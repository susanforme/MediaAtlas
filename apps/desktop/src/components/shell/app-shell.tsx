import { Link, Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import {
  FileText,
  FolderOpen,
  Layers3,
  ListTodo,
  Moon,
  Search,
  Settings2,
  Sun,
  Wrench,
} from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { NfoOpenListener } from "../../features/nfo/page";
import { setTheme, useTheme } from "../../lib/theme";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Toaster } from "../ui/sonner";
import { LibraryNavigation } from "./library-navigation";
import { useTaskEvents } from "./use-task-events";
import { useWindowChrome, type WindowChrome } from "./use-window-controls";
import { WindowControls } from "./window-controls";
import "./shell.css";

const workEntries = [
  ["/roots", "roots", FolderOpen],
  ["/tools", "videoTools", Wrench],
  ["/tasks", "tasks", ListTodo],
] as const;

function Sidebar({ chrome }: { chrome: WindowChrome }) {
  const { t } = useTranslation();
  return (
    <aside className="app-sidebar">
      <div
        className="app-brand"
        data-tauri-drag-region={chrome.platform ? "deep" : undefined}
      >
        <span className="app-brand-icon">
          <Layers3 size={23} aria-hidden="true" />
        </span>
        <span className="app-brand-name">MediaAtlas</span>
      </div>
      <nav className="app-nav" aria-label={t("local")}>
        <ul className="app-nav-list">
          <LibraryNavigation />
          <li className="app-nav-divider" aria-hidden="true" />
          {workEntries.map(([to, label, Icon]) => (
            <li key={to}>
              <Button asChild variant="ghost" className="app-nav-item">
                <Link
                  to={to}
                  activeOptions={{ exact: true, includeSearch: false }}
                  activeProps={{ className: "active" }}
                  aria-label={t(label)}
                >
                  <Icon size={20} aria-hidden="true" />
                  <span className="app-nav-label">{t(label)}</span>
                </Link>
              </Button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="app-sidebar-bottom">
        <Button asChild variant="ghost" className="app-nav-item">
          <Link
            to="/nfo"
            activeProps={{ className: "active" }}
            aria-label={t("openNfo")}
          >
            <FileText size={20} aria-hidden="true" />
            <span className="app-nav-label">{t("openNfo")}</span>
          </Link>
        </Button>
        <Button asChild variant="ghost" className="app-nav-item">
          <Link
            to="/settings"
            activeOptions={{ includeSearch: false }}
            activeProps={{ className: "active" }}
            aria-label={t("settings")}
          >
            <Settings2 size={20} aria-hidden="true" />
            <span className="app-nav-label">{t("settings")}</span>
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
  const [query, setQuery] = useState("");
  function submit(event: FormEvent) {
    event.preventDefault();
    void navigate({ to: "/library", search: { q: query.trim() || undefined } });
  }
  return (
    <header
      className="app-toolbar"
      data-tauri-drag-region={chrome.platform ? "deep" : undefined}
    >
      {pathname === "/library" ? (
        <div
          id="library-toolbar-search"
          className="app-toolbar-search"
          data-tauri-drag-region="false"
        />
      ) : (
        <form
          className="app-toolbar-search"
          onSubmit={submit}
          data-tauri-drag-region="false"
        >
          <Search className="app-search-icon" size={19} aria-hidden="true" />
          <Input
            aria-label={t("search")}
            placeholder={t("shellSearchPlaceholder")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
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
      <div className="app-toolbar-actions" data-tauri-drag-region="false">
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
  const pathname = useLocation({ select: (location) => location.pathname });
  const main = useRef<HTMLDivElement>(null);
  const chrome = useWindowChrome();
  useTaskEvents();
  useEffect(() => {
    // Route changes start at the content top; in-page field edits keep position.
    if (pathname && main.current) main.current.scrollTop = 0;
  }, [pathname]);
  useEffect(() => {
    document.documentElement.lang = i18n.resolvedLanguage ?? "zh-CN";
  }, [i18n.resolvedLanguage]);
  return (
    <div
      className="app-shell"
      data-window-platform={chrome.platform ?? undefined}
    >
      <Sidebar chrome={chrome} />
      <NfoOpenListener />
      <div className="app-main" ref={main}>
        <Toolbar chrome={chrome} />
        <main className="app-content">
          <Outlet />
        </main>
      </div>
      <Toaster />
    </div>
  );
}
