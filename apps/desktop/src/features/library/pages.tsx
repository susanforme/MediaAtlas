import { Link } from "@tanstack/react-router";
import { ChevronDown, FolderOpen } from "lucide-react";
import { useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { MediaKind } from "../../api/library";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { Textarea } from "../../components/ui/textarea";
import { RootCard } from "./root-card";
import { rootSupportsNfoExport } from "./library-model";
import { automaticRootSources } from "./root-source-model";
import { RootSourceReadiness } from "./root-source-readiness";
import type { ScannerSearch } from "./scanner-navigation";
import { useRootActivity } from "./use-root-activity";
import { useRootEditor } from "./use-root-editor";
import "./root-management.css";
import "./translations";
import "../adult/translations";
import { Button } from "../../components/ui/button";
import { Empty, ErrorNotice, Loading, PageHeading, ROOT_KINDS } from "./shared";

export function RootsPage({
  adult = false,
  onScan,
  onNavigateScan,
  initialAdd = false,
  initialKind,
}: {
  adult?: boolean;
  initialAdd?: boolean;
  initialKind?: MediaKind;
  onScan?: (rootId: string, taskId?: string) => void;
  onNavigateScan?: (search: ScannerSearch) => void;
  onTypeChange?: (adult: boolean) => void;
}) {
  const { t } = useTranslation();
  const advancedId = useId();
  const removeTrigger = useRef<HTMLButtonElement | null>(null);
  const addTrigger = useRef<HTMLButtonElement>(null);
  const {
    form,
    setForm,
    excludes,
    setExcludes,
    remove,
    setRemove,
    validation,
    roots,
    adultAvailable,
    adultVisible,
    save,
    deletion,
    edit,
    add,
    submit,
    closeEditor,
    changeName,
    changePath,
    chooseDirectory,
    advanced,
    setAdvanced,
    picking,
    pickerError,
    operation,
    scan,
    refresh,
    savedRoot,
    savedVersion,
    completed,
  } = useRootEditor(
    adult,
    (root, task, kind) => {
      if (onNavigateScan)
        onNavigateScan({
          root,
          task,
          ...(kind === "adult_video" ? { kind: "adult_video" as const } : {}),
        });
      else onScan?.(root, task);
    },
    initialAdd,
    initialKind,
  );
  const working = operation !== null;
  const activity = useRootActivity(roots.data, adultAvailable);
  return (
    <>
      <PageHeading title={t("roots")}>
        <Button
          ref={addTrigger}
          disabled={working || Boolean(form)}
          onClick={add}
        >
          {t("addRoot")}
        </Button>
      </PageHeading>
      {form && (
        <Card asChild>
          <form className="root-editor space-y-5" onSubmit={submit} noValidate>
            <h2 className="section-title">{t(form.id ? "edit" : "addRoot")}</h2>
            <div className="root-path-picker">
              <Button
                variant="outline"
                disabled={
                  working ||
                  picking ||
                  (form.kind === "adult_video" && !adultAvailable)
                }
                onClick={() => void chooseDirectory()}
              >
                <FolderOpen size={18} aria-hidden="true" />
                {t(
                  picking
                    ? "rootChoosingDirectory"
                    : form.path
                      ? "rootChangeDirectory"
                      : "rootChooseDirectory",
                )}
              </Button>
              <p
                className="min-w-0 break-all text-sm text-muted-foreground"
                aria-live="polite"
              >
                {form.path || t("rootNoDirectory")}
              </p>
            </div>
            {pickerError !== undefined && <ErrorNotice error={pickerError} />}
            <div className="grid gap-4 sm:grid-cols-2">
              <Label className="filter-field">
                <span>{t("rootName")}</span>
                <Input
                  value={form.name}
                  disabled={working}
                  onChange={(event) => changeName(event.target.value)}
                  required
                />
              </Label>
              <Label className="filter-field">
                <span>{t("rootType")}</span>
                <Select
                  disabled={working}
                  value={form.kind ?? "mixed"}
                  onChange={(event) => {
                    const kind = event.target.value === "mixed"
                      ? null
                      : (event.target.value as MediaKind);
                    setForm({
                      ...form,
                      kind,
                      metadata_providers: automaticRootSources(kind),
                      nfo_export_mode: rootSupportsNfoExport(kind)
                        ? form.nfo_export_mode ?? "online"
                        : "online",
                    });
                  }}
                >
                  {ROOT_KINDS.filter(
                    (kind) => adultVisible || kind !== "adult_video",
                  ).map((kind) => (
                    <option value={kind} key={kind}>
                      {t(`kind_${kind}`)}
                    </option>
                  ))}
                  <option value="mixed">{t("mixed")}</option>
                </Select>
              </Label>
            </div>
            {rootSupportsNfoExport(form.kind) && (
              <div className="space-y-2">
                <Label className="filter-field max-w-sm">
                  <span>{t("rootNfoExportMode")}</span>
                  <Select
                    value={form.nfo_export_mode ?? "online"}
                    disabled={working || picking}
                    onChange={(event) => setForm({
                      ...form,
                      nfo_export_mode: event.target.value === "offline" ? "offline" : "online",
                    })}
                  >
                    <option value="online">{t("rootNfoExportOnline")}</option>
                    <option value="offline">{t("rootNfoExportOffline")}</option>
                  </Select>
                </Label>
                {form.nfo_export_mode === "offline" && (
                  <p className="text-sm text-muted-foreground">{t("rootNfoExportOfflineHelp")}</p>
                )}
              </div>
            )}
            <RootSourceReadiness
              kind={form.kind}
              adultAvailable={adultAvailable}
              disabled={working || picking}
              value={form.metadata_providers ?? null}
              onValueChange={(metadata_providers) => setForm({ ...form, metadata_providers })}
            />
            <Button
              variant="ghost"
              size="sm"
              aria-expanded={advanced}
              aria-controls={advancedId}
              onClick={() => setAdvanced(!advanced)}
            >
              {t("rootAdvanced")}
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            {advanced && (
              <div
                id={advancedId}
                className="space-y-4 border-t border-border pt-4"
              >
                <Label className="filter-field">
                  <span>{t("rootPath")}</span>
                  <Input
                    value={form.path}
                    disabled={working || picking}
                    onChange={(event) => changePath(event.target.value)}
                    placeholder={t("rootManualPath")}
                    required
                  />
                </Label>
                <Label className="filter-field">
                  <span>{t("excludePatterns")}</span>
                  <Textarea
                    value={excludes}
                    disabled={working}
                    onChange={(event) => setExcludes(event.target.value)}
                    placeholder={t("excludeHelp")}
                    rows={3}
                  />
                </Label>
                <Label className="flex items-center gap-2">
                  <Checkbox
                    checked={form.enabled}
                    disabled={working}
                    onChange={(event) =>
                      setForm({ ...form, enabled: event.target.checked })
                    }
                  />
                  {t("enabled")}
                </Label>
              </div>
            )}
            {validation && <p role="alert">{t("invalidRoot")}</p>}
            {save.isError && <ErrorNotice error={save.error} />}
            {scan.isError && savedRoot && (
              <div className="space-y-3">
                <p role="status" className="text-sm">
                  {t("autoRootSavedFailed")}
                </p>
                <ErrorNotice error={scan.error} />
              </div>
            )}
            <div className="flex flex-wrap gap-3">
              <Button
                type="submit"
                variant="default"
                disabled={
                  working ||
                  picking ||
                  (form.kind === "adult_video" && !adultAvailable)
                }
              >
                {t(
                  working
                    ? "saving"
                    : form.enabled
                      ? scan.isError && savedVersion
                        ? "autoRootRetry"
                        : form.id
                          ? "autoRootSave"
                          : "autoRootAdd"
                      : "save",
                )}
              </Button>
              <Button variant="ghost" disabled={working} onClick={closeEditor}>
                {t("cancel")}
              </Button>
            </div>
          </form>
        </Card>
      )}
      {completed?.task && (
        <div className="mb-6 flex flex-wrap items-center gap-3">
          <Button variant="outline" asChild>
            <Link
              to="/library"
              search={{
                root: completed.root.id,
                ...(completed.root.kind === "adult_video"
                  ? { kind: "adult_video" as const }
                  : {}),
              }}
            >
              {t("autoBrowseLibrary")}
            </Link>
          </Button>
        </div>
      )}
      {remove && (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open && !deletion.isPending) setRemove(null);
          }}
        >
          <DialogContent
            aria-describedby={undefined}
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              (removeTrigger.current?.isConnected
                ? removeTrigger.current
                : addTrigger.current
              )?.focus();
            }}
          >
            <DialogTitle className="section-title">
              {t("removeRoot")}
            </DialogTitle>
            <p>{t("removeConfirm", { name: remove.name })}</p>
            {deletion.isError && <ErrorNotice error={deletion.error} />}
            <div className="mt-4 flex gap-3">
              <Button
                disabled={deletion.isPending}
                onClick={() => deletion.mutate(remove.id)}
              >
                {t("confirmRemove")}
              </Button>
              <Button variant="outline" onClick={() => setRemove(null)}>
                {t("cancel")}
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      )}
      {roots.isPending ? (
        <Loading />
      ) : roots.isError ? (
        <ErrorNotice error={roots.error} retry={() => void roots.refetch()} />
      ) : !roots.data.length ? (
        <Empty title={t("noRoots")} />
      ) : (
        <div className="root-grid">
          {roots.data.map((root) => (
            <RootCard
              key={root.id}
              root={root}
              task={activity.get(root.id)}
              refreshing={
                refresh.isPending && refresh.variables?.id === root.id
              }
              disabled={working || refresh.isPending}
              onEdit={() => edit(root)}
              onRefresh={() => refresh.mutate(root)}
              onRemove={(event) => {
                removeTrigger.current = event.currentTarget;
                deletion.reset();
                setRemove(root);
              }}
            />
          ))}
        </div>
      )}
      {refresh.isError && <ErrorNotice error={refresh.error} />}
    </>
  );
}

export { SettingsPage } from "../settings/page";
