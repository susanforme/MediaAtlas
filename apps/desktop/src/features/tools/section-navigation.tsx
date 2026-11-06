import { Link, useLocation } from "@tanstack/react-router";
import { Activity, Files, SearchCheck, Wrench } from "lucide-react";
import { useTranslation } from "react-i18next";
import { navItemClassName } from "../../components/shell/library-navigation";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import { type ScraperSearch, validateScraperSearch } from "../scraper/search";
import { type ToolsRouteSearch, type ToolsSection, toolsSectionLocation, validateToolsRouteSearch } from "./search";
import "./navigation";

const sections = [
  { value: "local", label: "toolsLocal", icon: Files },
  { value: "scraper", label: "toolsScraper", icon: SearchCheck },
  { value: "health", label: "toolsHealth", icon: Activity },
] as const;

export function ToolsSectionNavigation({ current, search = {}, sidebar = false, scraperSearch }: {
  current: ToolsSection; search?: ToolsRouteSearch; sidebar?: boolean; scraperSearch?: ScraperSearch;
}) {
  const { t } = useTranslation();
  return (
    <nav aria-label={t("toolsSections")} className={sidebar ? "mt-1 border-l border-border pl-3 [@media(max-width:1050px)]:hidden" : "mb-6 hidden overflow-x-auto border-b border-border [@media(max-width:1050px)]:block"}>
      <ul className={sidebar ? "grid list-none gap-1 p-0" : "flex min-w-max list-none gap-1 p-0"}>
        {sections.filter((section) => search.kind !== "adult_video" || section.value === "local").map(({ value, label, icon: Icon }) => (
          <li key={value}>
            <Button asChild variant="ghost" className={cn(
              sidebar ? "h-10 w-full justify-start rounded-lg px-3 text-[13px]" : "h-11 rounded-none border-b-2 border-transparent px-3",
              "aria-[current=page]:bg-muted aria-[current=page]:text-foreground aria-[current=page]:[&_svg]:text-ring",
              !sidebar && "aria-[current=page]:border-ring aria-[current=page]:bg-transparent",
            )}>
              <Link {...(value === "scraper" && scraperSearch ? { to: "/scraper" as const, search: scraperSearch } : toolsSectionLocation(value, search))} aria-current={current === value ? "page" : false} activeOptions={{ exact: true, includeSearch: true }}>
                <Icon size={16} aria-hidden="true" />{t(label)}
              </Link>
            </Button>
          </li>
        ))}
      </ul>
    </nav>
  );
}

export function ToolsSidebarNavigation() {
  const { t } = useTranslation();
  const location = useLocation();
  const active = location.pathname === "/tools" || location.pathname === "/scraper";
  const scraperSearch = location.pathname === "/scraper" ? validateScraperSearch(location.search) : undefined;
  const search: ToolsRouteSearch = location.pathname === "/tools" ? validateToolsRouteSearch(location.search) : { media: scraperSearch?.media };
  const current = location.pathname === "/scraper" ? "scraper" : search.view === "health" ? "health" : "local";
  return (
    <li>
      <Button asChild variant="ghost" className={cn(navItemClassName, active && "bg-muted text-foreground [&_svg]:text-ring")}>
        <Link to="/tools" search={{}} activeOptions={{ exact: true, includeSearch: true }} aria-label={t("videoTools")} data-section-active={active || undefined}>
          <Wrench size={20} aria-hidden="true" />
          <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">{t("videoTools")}</span>
        </Link>
      </Button>
      {active && <ToolsSectionNavigation current={current} search={search} scraperSearch={scraperSearch} sidebar />}
    </li>
  );
}
