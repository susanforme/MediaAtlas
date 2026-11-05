import { Link, useLocation } from "@tanstack/react-router";
import {
  ChevronDown,
  ChevronRight,
  Clapperboard,
  Film,
  Music2,
  Sparkles,
  Tv,
  Video,
} from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { usePrivacy } from "../../features/library/context";
import { LIBRARY_CATEGORIES } from "../../features/library/library-model";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";

export const navItemClassName =
  "flex h-11 w-full min-w-0 justify-start gap-3 rounded-[10px] px-[18px] py-0 text-left text-sm font-medium text-muted-foreground data-[section-active=true]:text-foreground data-[section-active=true]:[&_svg]:text-ring aria-[current=page]:bg-[var(--sidebar-active)] aria-[current=page]:text-foreground aria-[current=page]:[&_svg]:text-ring [&:hover]:bg-hover [&:hover]:text-foreground aria-[current=page]:[&:hover]:bg-[var(--sidebar-active-hover)] [@media(max-width:1050px)]:justify-center [@media(max-width:1050px)]:p-0";

const icons = {
  movie: Film,
  tv_series: Tv,
  anime: Sparkles,
  adult_video: Video,
};

export function LibraryNavigation() {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(true);
  const childrenId = useId();
  const { includeAdult } = usePrivacy();
  const pathname = useLocation({ select: (location) => location.pathname });
  const inLibrary =
    pathname === "/library" ||
    pathname === "/music" ||
    pathname.startsWith("/music/") ||
    pathname.startsWith("/media/");
  return (
    <li className="flex flex-col gap-1">
      <div className="group/nav-heading flex min-w-0 items-center rounded-[10px] has-[>[aria-current=page]]:bg-[var(--sidebar-active)] has-[>[aria-current=page]]:text-foreground [&:hover]:bg-hover [&:hover]:text-foreground has-[>[aria-current=page]]:[&:hover]:bg-[var(--sidebar-active-hover)] [@media(max-width:1050px)]:flex-col">
        <Button
          asChild
          variant="ghost"
          className={cn(
            navItemClassName,
            "w-auto flex-1 pr-1 bg-transparent [&:hover]:bg-transparent aria-[current=page]:bg-transparent aria-[current=page]:[&:hover]:bg-transparent group-[:hover]/nav-heading:text-foreground [@media(max-width:1050px)]:w-full [@media(max-width:1050px)]:p-0",
          )}
        >
          <Link
            to="/library"
            data-section-active={inLibrary || undefined}
            search={{ kind: undefined }}
            activeOptions={{ includeSearch: true, explicitUndefined: true }}
            activeProps={{ className: "active" }}
            aria-label={t("library")}
          >
            <Clapperboard size={20} aria-hidden="true" />
            <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">
              {t("library")}
            </span>
          </Link>
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-7 bg-transparent [&:hover]:bg-transparent group-[:hover]/nav-heading:text-foreground group-has-[>[aria-current=page]]/nav-heading:[&_svg]:text-ring [@media(max-width:1050px)]:h-[22px] [@media(max-width:1050px)]:w-full"
          aria-label={t(expanded ? "collapseLibrary" : "expandLibrary")}
          aria-expanded={expanded}
          aria-controls={childrenId}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? (
            <ChevronDown size={16} aria-hidden="true" />
          ) : (
            <ChevronRight size={16} aria-hidden="true" />
          )}
        </Button>
      </div>
      <ul
        id={childrenId}
        className="mx-0 mt-1 mb-2 flex list-none flex-col gap-1 p-0 [&[hidden]]:hidden [&>li]:min-w-0 [@media(max-width:1050px)]:mt-0 [@media(max-width:1050px)]:border-l-0 [@media(max-width:1050px)]:pl-0"
        hidden={!expanded}
      >
        {LIBRARY_CATEGORIES.filter(
          (kind) => kind !== "adult_video" || includeAdult,
        ).map((kind) => {
          if (kind === "music_release")
            return (
              <li key={kind}>
                <Button
                  asChild
                  variant="ghost"
                  className={navItemClassName}
                >
                  <Link
                    to="/music"
                    activeOptions={{ includeSearch: false }}
                    activeProps={{ className: "active" }}
                    aria-label={t("music")}
                  >
                    <Music2 size={20} aria-hidden="true" />
                    <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">
                      {t("music")}
                    </span>
                  </Link>
                </Button>
              </li>
            );
          const Icon = icons[kind as keyof typeof icons];
          if (!Icon) return null;
          return (
            <li key={kind}>
              <Button
                asChild
                variant="ghost"
                className={navItemClassName}
              >
                <Link
                  to="/library"
                  search={{ kind }}
                  activeOptions={{ includeSearch: true }}
                  activeProps={{ className: "active" }}
                  aria-label={t(`kind_${kind}`)}
                >
                  <Icon size={20} aria-hidden="true" />
                  <span className="app-nav-label overflow-hidden text-ellipsis [@media(max-width:1050px)]:hidden">
                    {t(`kind_${kind}`)}
                  </span>
                </Link>
              </Button>
            </li>
          );
        })}
      </ul>
    </li>
  );
}
