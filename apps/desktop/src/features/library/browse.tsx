import { Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import type { LibrarySearch } from "./browse-model";
import { LibraryActivity } from "./library-activity";
import { LibraryFilters } from "./library-filters";
import { LibraryPosterCard } from "./library-poster-card";
import { LibrarySourceRecovery } from "./library-source-recovery";
import { useLibraryBrowse } from "./use-library-browse";
import "./library.css";
import { NativeOpen } from "./native-open";
import { Empty, ErrorNotice, HealthBadges, Loading, Pager } from "./shared";

export { type LibrarySearch, validateLibrarySearch } from "./browse-model";

export function LibraryPage({
  search,
  onSearch,
  adult = false,
}: {
  search: LibrarySearch;
  onSearch: (patch: LibrarySearch, replace?: boolean) => void;
  adult?: boolean;
}) {
  const { t } = useTranslation();
  const {
    advanced,
    setAdvanced,
    history,
    setHistory,
    reset,
    media,
    roots,
    facets,
    filter,
    allowed,
    activity,
    adultAvailable,
  } = useLibraryBrowse(search, onSearch, adult);
  const [toolbarSearch, setToolbarSearch] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setToolbarSearch(document.getElementById("library-toolbar-search"));
  }, []);
  const searchInput = (
    <Label className="filter-field library-search">
      <span className={toolbarSearch ? "sr-only" : undefined}>
        {t("search")}
      </span>
      <Input
        type="search"
        placeholder={t("search")}
        value={search.q ?? ""}
        onChange={(event) => filter({ q: event.target.value || undefined })}
      />
    </Label>
  );
  const searchField = toolbarSearch
    ? createPortal(searchInput, toolbarSearch)
    : searchInput;
  const page = search.page ?? 1;
  if (!allowed)
    return adultAvailable ? <Loading /> : <Empty title={t("noResults")} />;
  return (
    <div className="library-page">
      <div className="library-page-heading">
        <h1>{t("library")}</h1>
        <div className="library-heading-actions">
          <Button asChild variant="ghost" size="sm">
            <Link to="/tools" search={adult ? { kind: "adult_video" } : {}}>
              {t("videoTools")}
            </Link>
          </Button>
          <Button asChild size="sm">
            <Link
              to="/roots"
              search={
                adult ? { add: true, kind: "adult_video" } : { add: true }
              }
            >
              <Plus size={16} aria-hidden="true" />
              {t("addRoot")}
            </Link>
          </Button>
        </div>
      </div>
      {!adult && (
        <div className="library-background-activity">
          <LibraryActivity activity={activity} />
        </div>
      )}
      {media.isSuccess &&
        activity.active.length === 0 &&
        !activity.attention.some(
          (task) => (task.unconfigured_count ?? 0) > 0,
        ) && (
          <LibrarySourceRecovery
            cards={media.data.items}
            roots={roots.data ?? []}
            search={search}
            adult={adult}
          />
        )}
      <LibraryFilters
        search={search}
        adult={adult}
        adultAvailable={adultAvailable}
        roots={roots.data ?? []}
        facets={facets.data}
        facetError={facets.isError}
        advanced={advanced}
        setAdvanced={setAdvanced}
        filter={filter}
        reset={reset}
        onView={(view) => onSearch({ ...search, view }, true)}
      >
        {searchField}
      </LibraryFilters>
      {media.isPending ? (
        <Loading />
      ) : media.isError ? (
        <ErrorNotice error={media.error} retry={() => void media.refetch()} />
      ) : media.data.items.length === 0 ? (
        <>
          <Empty
            title={
              search.q ||
              search.kind ||
              search.root ||
              search.health ||
              search.genre ||
              search.person ||
              search.studio ||
              search.year
                ? t("libraryNoResults")
                : roots.data?.length
                  ? t("libraryNoFiles")
                  : t("emptyLibrary")
            }
            body={
              search.q ||
              search.kind ||
              search.root ||
              search.health ||
              search.genre ||
              search.person ||
              search.studio ||
              search.year
                ? undefined
                : roots.data?.length
                  ? activity.active.length > 0
                    ? t("libraryRefreshReady")
                    : t("libraryNoFilesBody")
                  : t("emptyLibraryBody")
            }
          />
          {!adult && (
            <div className="mt-4 flex justify-center">
              <Button asChild variant="outline">
                <Link
                  to="/roots"
                  search={roots.data?.length ? {} : { add: true }}
                >
                  {t(roots.data?.length ? "libraryRefreshManage" : "addRoot")}
                </Link>
              </Button>
            </div>
          )}
        </>
      ) : search.view === "list" ? (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t("titleLabel")}</th>
                <th>{t("type")}</th>
                <th>{t("year")}</th>
                <th>{t("health")}</th>
              </tr>
            </thead>
            <tbody>
              {media.data.items.map((card) => (
                <tr key={card.media.id}>
                  <td>
                    <NativeOpen mediaId={card.media.id} adult={adult}>
                      {card.media.title}
                    </NativeOpen>
                    <Button asChild variant="ghost">
                      <Link
                        to="/media/$mediaId"
                        params={{ mediaId: card.media.id }}
                        search={adult ? { kind: "adult_video" } : {}}
                        className="font-medium text-ring"
                      >
                        {t("detailsAction")}
                      </Link>
                    </Button>
                  </td>
                  <td>{t(`kind_${card.media.kind}`)}</td>
                  <td>{card.media.year ? card.media.year : t("none")}</td>
                  <td>
                    <HealthBadges health={card.health} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="poster-grid library-poster-grid">
          {media.data.items.map((card) => (
            <LibraryPosterCard key={card.media.id} card={card} adult={adult} />
          ))}
        </div>
      )}
      {media.isSuccess &&
        media.data.items.length > 0 &&
        (page > 1 || media.data.next_cursor) && (
          <Pager
            page={page}
            hasNext={Boolean(media.data.next_cursor)}
            previous={() => {
              const prev = page - 1;
              onSearch({
                ...search,
                cursor: history[prev - 1],
                page: history[prev - 1] === undefined ? 1 : prev,
              });
            }}
            next={() => {
              if (!media.data.next_cursor) return;
              setHistory((old) => {
                const next = [...old];
                next[page] = media.data.next_cursor ?? undefined;
                return next;
              });
              onSearch({
                ...search,
                cursor: media.data.next_cursor ?? undefined,
                page: page + 1,
              });
            }}
          />
        )}
    </div>
  );
}
