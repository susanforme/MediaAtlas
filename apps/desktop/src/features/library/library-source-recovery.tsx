import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { MediaCard, RootView } from "../../api/library";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { validateProviderSetupSearch } from "../settings/setup-search";
import type { LibrarySearch } from "./browse-model";
import "./library-status-translations";

/** Persisted media may outlive the task list; keep source setup reachable. */
export function LibrarySourceRecovery({
  cards,
  roots,
  search,
  adult,
}: {
  cards: MediaCard[];
  roots: RootView[];
  search: LibrarySearch;
  adult: boolean;
}) {
  const { t } = useTranslation();
  if (!cards.some((card) => card.health.includes("metadata_unconfigured")))
    return null;
  const root = search.root ?? (roots.length === 1 ? roots[0].id : undefined);
  const kind = adult ? "adult_video" : search.kind;
  return (
    <Alert asChild>
      <section className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <p role="status">{t("libraryRefreshUnconfigured")}</p>
        <Button asChild size="sm">
          <Link
            to="/settings"
            search={validateProviderSetupSearch({ from: "library", root, kind })}
          >
            {t(
              root
                ? "libraryRefreshConfigureAction"
                : "libraryRefreshConfigureOnly",
            )}
          </Link>
        </Button>
      </section>
    </Alert>
  );
}
