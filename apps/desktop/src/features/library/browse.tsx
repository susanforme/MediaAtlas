import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ImageOff, LayoutGrid, List } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type BrowseQuery,
  defaultBrowse,
  type FacetOption,
  libraryApi,
  type MediaKind,
} from "../../api/library";
import { Button } from "../../components/ui/button";
import {
  ALL_KINDS,
  Empty,
  ErrorNotice,
  HEALTH,
  HealthBadges,
  Loading,
  PageHeading,
  Pager,
  useFormat,
} from "./shared";

export interface LibrarySearch {
  root?: string;
  kind?: MediaKind;
  q?: string;
  health?: string;
  genre?: string;
  person?: string;
  studio?: string;
  year?: number;
  sort?: string;
  cursor?: string;
  page?: number;
  view?: "wall" | "list";
}
export function validateLibrarySearch(
  value: Record<string, unknown>,
): LibrarySearch {
  const out: LibrarySearch = {};
  for (const key of [
    "root",
    "q",
    "health",
    "genre",
    "person",
    "studio",
    "sort",
    "cursor",
  ] as const)
    if (typeof value[key] === "string" && value[key])
      out[key] = value[key] as string;
  if (ALL_KINDS.includes(value.kind as MediaKind))
    out.kind = value.kind as MediaKind;
  const year = Number(value.year);
  if (value.year && Number.isInteger(year)) out.year = year;
  const page = Number(value.page);
  if (Number.isInteger(page) && page > 0) out.page = page;
  if (value.view === "list" || value.view === "wall") out.view = value.view;
  return out;
}
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
    <label className="filter-field">
      <span>{label}</span>
      <select
        value={value ?? ""}
        onChange={(event) => change(event.target.value)}
      >
        <option value="">{label}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.value} ({number(option.count)})
          </option>
        ))}
      </select>
    </label>
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
  const { number } = useFormat();
  const [advanced, setAdvanced] = useState(false);
  const [history, setHistory] = useState<(string | undefined)[]>([undefined]);
  const access = false;
  const query: BrowseQuery = {
    ...defaultBrowse(access),
    root_id: search.root ?? null,
    kind: adult
      ? "adult_video"
      : search.kind === "adult_video"
        ? null
        : (search.kind ?? null),
    query: search.q ?? null,
    health: search.health ?? null,
    genre: search.genre ?? null,
    person: search.person ?? null,
    studio: search.studio ?? null,
    year: search.year ?? null,
    sort: search.sort ?? "updated_desc",
    cursor: search.cursor ?? null,
  };
  const allowed = !adult;
  const roots = useQuery({
    queryKey: ["roots", access],
    queryFn: () => libraryApi.roots(access),
    enabled: allowed,
  });
  const media = useQuery({
    queryKey: ["media", query],
    queryFn: () => libraryApi.media(query),
    enabled: allowed,
  });
  const facets = useQuery({
    queryKey: ["facets", { ...query, cursor: null }],
    queryFn: () => libraryApi.facets({ ...query, cursor: null }),
    enabled: allowed,
  });
  function filter(patch: LibrarySearch) {
    setHistory([undefined]);
    onSearch({ ...search, ...patch, cursor: undefined, page: 1 });
  }
  const page = search.page ?? 1;
  if (!allowed) return <Empty title={t("privacyHidden")} />;
  return (
    <>
      <PageHeading title={adult ? t("adultLibrary") : t("library")}>
        <div className="flex gap-2">
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
      <div className="panel mb-6">
        <div className="filter-grid">
          <label className="filter-field">
            <span>{t("search")}</span>
            <input
              type="search"
              value={search.q ?? ""}
              onChange={(event) =>
                filter({ q: event.target.value || undefined })
              }
              placeholder={t("search")}
            />
          </label>
          <label className="filter-field">
            <span>{t("root")}</span>
            <select
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
            </select>
          </label>
          {!adult && (
            <label className="filter-field">
              <span>{t("type")}</span>
              <select
                value={search.kind ?? ""}
                onChange={(event) =>
                  filter({
                    kind: (event.target.value || undefined) as
                      | MediaKind
                      | undefined,
                  })
                }
              >
                <option value="">{t("allKinds")}</option>
                {ALL_KINDS.filter((k) => k !== "adult_video").map((kind) => (
                  <option value={kind} key={kind}>
                    {t(`kind_${kind}`)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="filter-field">
            <span>{t("health")}</span>
            <select
              value={search.health ?? ""}
              onChange={(event) =>
                filter({ health: event.target.value || undefined })
              }
            >
              <option value="">{t("allHealth")}</option>
              {HEALTH.map((health) => (
                <option key={health} value={health}>
                  {t(`health_${health}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="filter-field">
            <span>{t("sort")}</span>
            <select
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
            </select>
          </label>
        </div>
        <div className="mt-4 flex flex-wrap gap-3">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setAdvanced(!advanced)}
          >
            {t(advanced ? "showLess" : "showMore")}
          </Button>
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
        {advanced && (
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
        <Empty
          title={
            search.q || search.kind || search.root || search.health
              ? t("noResults")
              : t("emptyLibrary")
          }
          body={t("emptyLibraryBody")}
        />
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
                    <Link
                      to="/media/$mediaId"
                      params={{ mediaId: card.media.id }}
                      className="font-medium text-primary"
                    >
                      {card.media.title}
                    </Link>
                  </td>
                  <td>{t(`kind_${card.media.kind}`)}</td>
                  <td>
                    {card.media.year ? number(card.media.year) : t("none")}
                  </td>
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
            <Link
              to="/media/$mediaId"
              params={{ mediaId: card.media.id }}
              key={card.media.id}
              className="poster-card"
            >
              <div className="poster-image">
                {card.poster_uri?.startsWith("data:image/") ? (
                  <img
                    src={card.poster_uri}
                    alt={card.media.title}
                    loading="lazy"
                  />
                ) : (
                  <div className="poster-placeholder">
                    <ImageOff size={28} aria-hidden="true" />
                    <span>{t("noPoster")}</span>
                  </div>
                )}
              </div>
              <div className="p-4">
                <h2 className="truncate font-semibold" title={card.media.title}>
                  {card.media.title}
                </h2>
                <p className="mt-1 text-xs text-muted-foreground">
                  {card.media.year ? number(card.media.year) : t("unknownYear")}{" "}
                  · {t(`kind_${card.media.kind}`)}
                </p>
                <div className="mt-3">
                  <HealthBadges health={card.health} />
                </div>
              </div>
            </Link>
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
