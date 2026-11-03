import { Link } from "@tanstack/react-router";
import { LayoutGrid, List } from "lucide-react";
import { useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { FacetOption, MediaKind } from "../../api/library";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import type { LibrarySearch } from "./browse-model";
import { LibraryActivity } from "./library-activity";
import { LIBRARY_CATEGORIES } from "./library-model";
import { MediaArtwork } from "./media-artwork";
import { useLibraryBrowse } from "./use-library-browse";
import "./library.css";
import { NativeOpen } from "./native-open";
import {
  Empty,
  ErrorNotice,
  HEALTH,
  HealthBadges,
  Loading,
  PageHeading,
  Pager,
  useFormat,
} from "./shared";

export { type LibrarySearch, validateLibrarySearch } from "./browse-model";

function FacetSelect({
  label,
  value,
  options,
  change,
}: {
  label: string;
  value?: string;
  options: FacetOption[];
  change: (value: string) => void;
}) {
  const { number } = useFormat();
  return (
    <Label className="filter-field">
      <span>{label}</span>
      <Select
        value={value ?? ""}
        onChange={(event) => change(event.target.value)}
      >
        <option value="">{label}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.value} ({number(option.count)})
          </option>
        ))}
      </Select>
    </Label>
  );
}
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
    <Label className="filter-field">
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
    <>
      <PageHeading title={t("library")}>
        <div className="flex min-w-0 flex-wrap gap-2">
          <Button asChild variant="ghost">
            <Link
              to="/tools"
              search={adult ? { kind: "adult_video" } : {}}
              className="action-link"
            >
              {t("videoTools")}
            </Link>
          </Button>
          <Button
            variant={search.view !== "list" ? "default" : "outline"}
            aria-label={t("wall")}
            onClick={() => onSearch({ ...search, view: "wall" }, true)}
          >
            <LayoutGrid size={16} aria-hidden="true" />
          </Button>
          <Button
            variant={search.view === "list" ? "default" : "outline"}
            aria-label={t("list")}
            onClick={() => onSearch({ ...search, view: "list" }, true)}
          >
            <List size={16} aria-hidden="true" />
          </Button>
        </div>
      </PageHeading>
      {!adult && <LibraryActivity activity={activity} />}
      <div className="panel mb-6">
        <div className="filter-grid">
          {searchField}
          <Label className="filter-field">
            <span>{t("root")}</span>
            <Select
              value={search.root ?? ""}
              onChange={(event) =>
                filter({ root: event.target.value || undefined })
              }
            >
              <option value="">{t("allRoots")}</option>
              {roots.data
                ?.filter((r) =>
                  adult
                    ? r.kind === "adult_video" || r.kind === null
                    : r.kind !== "adult_video",
                )
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name}
                  </option>
                ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("type")}</span>
            <Select
              value={search.kind ?? ""}
              onChange={(event) =>
                filter({
                  kind: (event.target.value || undefined) as
                    | MediaKind
                    | undefined,
                  root: undefined,
                  genre: undefined,
                  person: undefined,
                  studio: undefined,
                  year: undefined,
                })
              }
            >
              <option value="">{t("allKinds")}</option>
              {LIBRARY_CATEGORIES.filter(
                (k) => k !== "adult_video" || adultAvailable,
              ).map((kind) => (
                <option value={kind} key={kind}>
                  {t(
                    kind === "music_release"
                      ? "libraryKindMusic"
                      : `kind_${kind}`,
                  )}
                </option>
              ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("health")}</span>
            <Select
              value={search.health ?? ""}
              onChange={(event) =>
                filter({ health: event.target.value || undefined })
              }
            >
              <option value="">{t("allHealth")}</option>
              {HEALTH.filter(
                (health) => !adult || !health.startsWith("metadata_"),
              ).map((health) => (
                <option key={health} value={health}>
                  {t(`health_${health}`)}
                </option>
              ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("sort")}</span>
            <Select
              value={search.sort ?? "updated_desc"}
              onChange={(event) => filter({ sort: event.target.value })}
            >
              {["title_asc", "year_desc", "added_desc", "updated_desc"].map(
                (sort) => (
                  <option key={sort} value={sort}>
                    {t(`sort_${sort}`)}
                  </option>
                ),
              )}
            </Select>
          </Label>
        </div>
        <div className="mt-4 flex flex-wrap gap-3">
          {!adult && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setAdvanced(!advanced)}
            >
              {t(advanced ? "showLess" : "showMore")}
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              setHistory([undefined]);
              onSearch({ view: search.view });
            }}
          >
            {t("clearFilters")}
          </Button>
        </div>
        {advanced && !adult && (
          <div className="filter-grid mt-5">
            <FacetSelect
              label={t("allGenres")}
              options={facets.data?.genres ?? []}
              value={search.genre}
              change={(v) => filter({ genre: v || undefined })}
            />
            <FacetSelect
              label={t("allPeople")}
              options={facets.data?.people ?? []}
              value={search.person}
              change={(v) => filter({ person: v || undefined })}
            />
            <FacetSelect
              label={t("allStudios")}
              options={facets.data?.studios ?? []}
              value={search.studio}
              change={(v) => filter({ studio: v || undefined })}
            />
            <FacetSelect
              label={t("allYears")}
              options={facets.data?.years ?? []}
              value={search.year?.toString()}
              change={(v) => filter({ year: v ? Number(v) : undefined })}
            />
          </div>
        )}
        {facets.isError && (
          <p className="mt-3 text-sm text-muted-foreground">
            {t("operationFailed")}
          </p>
        )}
      </div>
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
                <Link to="/roots">
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
        <div className="poster-grid">
          {media.data.items.map((card) => (
            <article key={card.media.id} className="poster-card">
              <NativeOpen
                mediaId={card.media.id}
                adult={adult}
                label={card.media.title}
                className="h-auto w-full flex-col items-stretch overflow-hidden rounded-none bg-transparent p-0 text-left text-foreground hover:bg-muted"
              >
                <div className="poster-image">
                  <MediaArtwork
                    uri={card.poster_uri}
                    title={card.media.title}
                  />
                </div>
                <span
                  className="block truncate p-4 font-semibold"
                  title={card.media.title}
                >
                  {card.media.title}
                </span>
              </NativeOpen>
              <div className="p-4">
                <Button asChild variant="ghost">
                  <Link
                    to="/media/$mediaId"
                    params={{ mediaId: card.media.id }}
                    search={adult ? { kind: "adult_video" } : {}}
                    className="action-link"
                  >
                    {t("detailsAction")}
                  </Link>
                </Button>
                <p className="mt-1 text-xs text-muted-foreground">
                  {card.media.year ? card.media.year : t("unknownYear")} ·{" "}
                  {t(`kind_${card.media.kind}`)}
                </p>
                <div className="mt-3">
                  <HealthBadges health={card.health} />
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
      {media.isSuccess && (
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
    </>
  );
}
