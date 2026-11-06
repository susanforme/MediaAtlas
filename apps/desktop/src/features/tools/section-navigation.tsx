import { Link, useLocation } from "@tanstack/react-router";
import { Activity, ChevronDown, ChevronRight, Files, SearchCheck, Wrench } from "lucide-react";
import { useId, useState } from "react";
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
  current?: ToolsSection; search?: ToolsRouteSearch; sidebar?: boolean; scraperSearch?: ScraperSearch;
}) {
  const { t } = useTranslation();
  return (
    <nav aria-label={t("toolsSections")} className={sidebar ? "mt-1 mb-2 [@media(max-width:1050px)]:mt-0" : "mb-6 hidden overflow-x-auto border-b border-border [@media(max-width:1050px)]:block"}>
      <ul className={sidebar ? "m-0 flex list-none flex-col gap-1 p-0 [&>li]:min-w-0" : "flex min-w-max list-none gap-1 p-0"}>
        {sections.filter((section) => search.kind !== "adult_video" || section.value === "local").map(({ value, label, icon: Icon }) => (
          <li key={value}>
            <Button asChild variant="ghost" className={cn(
              sidebar ? navItemClassName : "h-11 rounded-none border-b-2 border-transparent px-3 aria-[current=page]:border-ring aria-[current=page]:bg-transparent aria-[current=page]:text-foreground aria-[current=page]:[&_svg]:text-ring",
            )}>
              <Link {...(value === "scraper" && scraperSearch ? { to: "/scraper" as const, search: scraperSearch } : toolsSectionLocation(value, search))} aria-label={t(label)} aria-current={current === value ? "page" : false} activeOptions={{ exact: true, includeSearch: true }}>
                <Icon size={sidebar ? 20 : 16} aria-hidden="true" />
                <span className={sidebar ? "app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden" : undefined}>{t(label)}</span>
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
  const [expanded, setExpanded] = useState(true);
  const childrenId = useId();
  const location = useLocation();
  const active = location.pathname === "/tools" || location.pathname === "/scraper";
  const scraperSearch = location.pathname === "/scraper" ? validateScraperSearch(location.search) : undefined;
  const search: ToolsRouteSearch = location.pathname === "/tools" ? validateToolsRouteSearch(location.search) : { media: scraperSearch?.media };
  const current = !active ? undefined : location.pathname === "/scraper" ? "scraper" : search.view === "health" ? "health" : "local";
  return (
    <li className="flex flex-col gap-1">
      <div className="group/nav-heading flex min-w-0 items-center rounded-[10px] has-[>[aria-current=page]]:bg-[var(--sidebar-active)] has-[>[aria-current=page]]:text-foreground [&:hover]:bg-hover [&:hover]:text-foreground has-[>[aria-current=page]]:[&:hover]:bg-[var(--sidebar-active-hover)] [@media(max-width:1050px)]:flex-col">
        <Button asChild variant="ghost" className={cn(navItemClassName, "w-auto flex-1 pr-1 bg-transparent [&:hover]:bg-transparent aria-[current=page]:bg-transparent aria-[current=page]:[&:hover]:bg-transparent group-[:hover]/nav-heading:text-foreground [@media(max-width:1050px)]:w-full [@media(max-width:1050px)]:p-0")}>
          <Link to="/tools" search={{}} activeOptions={{ exact: true, includeSearch: true }} aria-label={t("videoTools")} data-section-active={active || undefined}>
            <Wrench size={20} aria-hidden="true" />
            <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">{t("videoTools")}</span>
          </Link>
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-7 bg-transparent [&:hover]:bg-transparent group-[:hover]/nav-heading:text-foreground group-has-[>[aria-current=page]]/nav-heading:[&_svg]:text-ring [@media(max-width:1050px)]:h-[22px] [@media(max-width:1050px)]:w-full"
          aria-label={t(expanded ? "collapseTools" : "expandTools")}
          aria-expanded={expanded}
          aria-controls={childrenId}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown size={16} aria-hidden="true" /> : <ChevronRight size={16} aria-hidden="true" />}
        </Button>
      </div>
      <div id={childrenId} hidden={!expanded} className="[&[hidden]]:hidden">
        <ToolsSectionNavigation current={current} search={search} scraperSearch={scraperSearch} sidebar />
      </div>
    </li>
  );
}
