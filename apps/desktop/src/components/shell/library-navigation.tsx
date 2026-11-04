import { Link, useLocation } from "@tanstack/react-router";
import { Clapperboard, Film, Music2, Sparkles, Tv, Video } from "lucide-react";
import { useTranslation } from "react-i18next";
import { usePrivacy } from "../../features/library/context";
import { LIBRARY_CATEGORIES } from "../../features/library/library-model";
import { Button } from "../ui/button";

const icons = {
  movie: Film,
  tv_series: Tv,
  anime: Sparkles,
  adult_video: Video,
};

export function LibraryNavigation() {
  const { t } = useTranslation();
  const { includeAdult } = usePrivacy();
  const pathname = useLocation({ select: (location) => location.pathname });
  return (
    <div className="app-nav-group">
      <Button asChild variant="ghost" className="app-nav-item">
        <Link
          to="/library"
          className={
            pathname === "/music" || pathname.startsWith("/media/")
              ? "active"
              : undefined
          }
          search={{}}
          activeOptions={{ includeSearch: false }}
          activeProps={{ className: "active" }}
          aria-label={t("library")}
        >
          <Clapperboard size={20} aria-hidden="true" />
          <span className="app-nav-label">{t("library")}</span>
        </Link>
      </Button>
      <div className="app-nav-children">
        {LIBRARY_CATEGORIES.filter(
          (kind) => kind !== "adult_video" || includeAdult,
        ).map((kind) => {
          if (kind === "music_release")
            return (
              <Button
                asChild
                variant="ghost"
                className="app-nav-item app-nav-child"
                key={kind}
              >
                <Link
                  to="/music"
                  activeOptions={{ includeSearch: false }}
                  activeProps={{ className: "active" }}
                  aria-label={t("music")}
                >
                  <Music2 size={17} aria-hidden="true" />
                  <span className="app-nav-label">{t("music")}</span>
                </Link>
              </Button>
            );
          const Icon = icons[kind as keyof typeof icons];
          if (!Icon) return null;
          return (
            <Button
              asChild
              variant="ghost"
              className="app-nav-item app-nav-child"
              key={kind}
            >
              <Link
                to="/library"
                search={{ kind }}
                activeOptions={{ includeSearch: true }}
                activeProps={{ className: "active" }}
                aria-label={t(`kind_${kind}`)}
              >
                <Icon size={17} aria-hidden="true" />
                <span className="app-nav-label">{t(`kind_${kind}`)}</span>
              </Link>
            </Button>
          );
        })}
      </div>
    </div>
  );
}
