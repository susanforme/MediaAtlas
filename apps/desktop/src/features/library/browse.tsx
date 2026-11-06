import { Link } from "@tanstack/react-router";
import { Plus, Search } from "lucide-react";
import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "../../components/ui/table";
import { hasLibraryFilters, type LibrarySearch } from "./browse-model";
import { LibraryActivity } from "./library-activity";
import { LibraryFilters } from "./library-filters";
import { LibraryPosterCard } from "./library-poster-card";
import { LibrarySourceRecovery } from "./library-source-recovery";
import { useLibraryBrowse } from "./use-library-browse";
import { Empty, ErrorNotice, HealthBadges, Loading, Pager } from "./shared";

export { type LibrarySearch, validateLibrarySearch } from "./browse-model";

function LibrarySearchForm({
  search,
  onSearch,
  inputRef,
}: {
  search: LibrarySearch;
  onSearch: (patch: LibrarySearch) => void;
  inputRef: RefObject<HTMLInputElement | null>;
}) {
  const { t } = useTranslation();
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    form.current?.reset();
  }, [search]);
  return (
    <form
      ref={form}
      role="search"
      aria-label={t("search")}
      data-slot="library-search"
      className="relative flex w-full min-w-0 items-center [&_[data-slot=input]]:pl-[43px] [&_[data-slot=input]]:pr-11 [&>[data-slot=button]]:absolute [&>[data-slot=button]]:right-0.5 [&>[data-slot=button]]:rounded-full"
      onSubmit={(event) => {
        event.preventDefault();
        const query = new FormData(event.currentTarget).get("q");
        onSearch({
          q: typeof query === "string" ? query.trim() || undefined : undefined,
        });
      }}
    >
      <Search className="pointer-events-none absolute left-4 z-1 text-muted-foreground" size={19} aria-hidden="true" />
      <Input
        ref={inputRef}
        type="search"
        name="q"
        aria-label={t("search")}
        aria-keyshortcuts="Control+f Meta+f"
        placeholder={t("shellSearchPlaceholder")}
        defaultValue={search.q ?? ""}
      />
      <Button type="submit" variant="ghost" size="icon" aria-label={t("search")}>
        <Search size={17} aria-hidden="true" />
      </Button>
    </form>
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
  const searchInputRef = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => {
    setToolbarSearch(document.getElementById("library-toolbar-search"));
  }, []);
  const searchInput = (
    <LibrarySearchForm search={search} onSearch={filter} inputRef={searchInputRef} />
  );
  const searchField = toolbarSearch
    ? createPortal(searchInput, toolbarSearch)
    : searchInput;
  const page = search.page ?? 1;
  const filtered = hasLibraryFilters(search);
  const scoped = filtered || search.kind || search.root;
  if (!allowed)
    return adultAvailable ? <Loading /> : <Empty title={t("noResults")} />;
  return (
    <div className="min-w-0">
      <div className="mb-[18px] flex flex-wrap items-center justify-between gap-3 [&_h1]:text-[26px] [&_h1]:leading-[1.25] [&_h1]:font-semibold [&_h1]:tracking-[-0.025em] [@media(width<=760px)]:mb-3.5 [@media(width<=760px)]:[&_h1]:text-[23px]">
        <h1>{t("library")}</h1>
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
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
        <div className="[&>section]:mb-4 [&>section]:px-4 [&>section]:py-3">
          <LibraryActivity activity={activity} search={search} />
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
              scoped
                ? t("libraryNoResults")
                : roots.data?.length
                  ? t("libraryNoFiles")
                  : t("emptyLibrary")
            }
            body={
              scoped
                ? undefined
                : roots.data?.length
                  ? activity.active.length > 0
                    ? t("libraryRefreshReady")
                    : t("libraryNoFilesBody")
                  : t("emptyLibraryBody")
            }
          />
          {filtered ? (
            <div className="mt-4 flex justify-center">
              <Button
                variant="outline"
                onClick={() => {
                  searchInputRef.current?.focus();
                  reset();
                }}
              >
                {t("libraryResetSearch")}
              </Button>
            </div>
          ) : !adult && (
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
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("titleLabel")}</TableHead>
              <TableHead>{t("type")}</TableHead>
              <TableHead>{t("year")}</TableHead>
              <TableHead>{t("health")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {media.data.items.map((card) => (
              <TableRow key={card.media.id}>
                <TableCell>
                  <Button
                    asChild
                    variant="link"
                    className="h-auto justify-start whitespace-normal px-0 text-left text-foreground"
                  >
                    <Link
                      to="/media/$mediaId"
                      params={{ mediaId: card.media.id }}
                      search={adult ? { kind: "adult_video" } : {}}
                      aria-label={t("libraryPosterOpen", { title: card.media.title })}
                    >
                      {card.media.title}
                    </Link>
                  </Button>
                </TableCell>
                <TableCell>{t(`kind_${card.media.kind}`)}</TableCell>
                <TableCell>{card.media.year ? card.media.year : t("none")}</TableCell>
                <TableCell>
                  <HealthBadges health={card.health} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-[18px] gap-y-[22px] [@media(width<=760px)]:grid-cols-[repeat(auto-fill,minmax(min(100%,125px),1fr))] [@media(width<=760px)]:gap-x-3 [@media(width<=760px)]:gap-y-[18px]">
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
