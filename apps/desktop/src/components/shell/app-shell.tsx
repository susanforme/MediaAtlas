import { Link, Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import {
  Clapperboard,
  FolderOpen,
  Layers3,
  ListTodo,
  Music2,
  Search,
  Settings2,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Select } from "../ui/select";
import { useTaskEvents } from "./use-task-events";
import "./shell.css";

const browseEntries = [
  ["/library", "library", Clapperboard],
  ["/music", "music", Music2],
] as const;
const workEntries = [
  ["/roots", "roots", FolderOpen],
  ["/tools", "videoTools", Wrench],
  ["/tasks", "tasks", ListTodo],
] as const;

function Sidebar() {
  const { t } = useTranslation();
  return (
    <aside className="app-sidebar">
      <div className="app-brand">
        <span className="app-brand-icon">
          <Layers3 size={23} aria-hidden="true" />
        </span>
        <span className="app-brand-name">MediaAtlas</span>
      </div>
      <nav className="app-nav" aria-label={t("local")}>
        {[browseEntries, workEntries].map((group, index) => (
          <div className="app-nav-group" key={group[0][0]}>
            {index > 0 && <div className="app-nav-divider" />}
            {group.map(([to, label, Icon]) => (
              <Button asChild variant="ghost" key={to} className="app-nav-item">
                <Link
                  to={to}
                  activeOptions={{ exact: true }}
                  activeProps={{ className: "active" }}
                  aria-label={t(label)}
                >
                  <Icon size={20} aria-hidden="true" />
                  <span className="app-nav-label">{t(label)}</span>
                </Link>
              </Button>
            ))}
          </div>
        ))}
      </nav>
      <div className="app-sidebar-bottom">
        <Button asChild variant="ghost" className="app-nav-item">
          <Link
            to="/settings"
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

function Toolbar() {
  const { t, i18n } = useTranslation();
  const pathname = useLocation({ select: (location) => location.pathname });
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  function submit(event: FormEvent) {
    event.preventDefault();
    void navigate({ to: "/library", search: { q: query.trim() || undefined } });
  }
  return (
    <header className="app-toolbar">
      {pathname === "/library" ? (
        <div id="library-toolbar-search" className="app-toolbar-search" />
      ) : (
        <form className="app-toolbar-search" onSubmit={submit}>
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
      <div className="app-toolbar-actions">
        <ShieldCheck className="app-local-icon" size={19} aria-hidden="true" />
        <Select
          className="app-language"
          aria-label={t("language")}
          value={i18n.resolvedLanguage ?? "zh-CN"}
          onChange={(event) => void i18n.changeLanguage(event.target.value)}
        >
          <option value="zh-CN">简体中文</option>
          <option value="en-US">English</option>
        </Select>
      </div>
    </header>
  );
}

export function AppShell() {
  const { i18n } = useTranslation();
  useTaskEvents();
  useEffect(() => {
    document.documentElement.lang = i18n.resolvedLanguage ?? "zh-CN";
  }, [i18n.resolvedLanguage]);
  return (
    <div className="app-shell">
      <Sidebar />
      <div className="app-main">
        <Toolbar />
        <main className="app-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
