import { Link } from "@tanstack/react-router";
import { ChevronDown, FolderOpen } from "lucide-react";
import { useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { MediaKind } from "../../api/library";
import { Checkbox } from "../../components/ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { Textarea } from "../../components/ui/textarea";
import type { ScannerSearch } from "./scanner-navigation";
import { useRootEditor } from "./use-root-editor";
import "./translations";
import "../adult/translations";
import { Button } from "../../components/ui/button";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  ROOT_KINDS,
  useFormat,
} from "./shared";

export { DashboardPage } from "../home/page";

export function RootsPage({
  adult = false,
  onScan,
  onNavigateScan,
}: {
  adult?: boolean;
  onScan?: (rootId: string, taskId?: string) => void;
  onNavigateScan?: (search: ScannerSearch) => void;
}) {
  const { t } = useTranslation();
  const { date } = useFormat();
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
    save,
    deletion,
    edit,
    submit,
    persist,
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
    savedRoot,
    savedVersion,
    completed,
  } = useRootEditor(adult, (root, task) => {
    if (adult) onScan?.(root, task);
    else onNavigateScan?.({ root, task });
  });
  const working = operation !== null;
  return (
    <>
      <PageHeading title={t("roots")}>
        <Button ref={addTrigger} disabled={working} onClick={() => edit()}>
          {t("addRoot")}
        </Button>
      </PageHeading>
      {form && (
        <form className="panel mb-6 space-y-4" onSubmit={submit} noValidate>
          <h2 className="section-title">{t(form.id ? "edit" : "addRoot")}</h2>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              variant="outline"
              disabled={working || picking}
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
              onChange={(event) =>
                setForm({
                  ...form,
                  kind:
                    event.target.value === "mixed"
                      ? null
                      : (event.target.value as MediaKind),
                })
              }
            >
              {ROOT_KINDS.filter((kind) =>
                adult ? kind === "adult_video" : kind !== "adult_video",
              ).map((kind) => (
                <option value={kind} key={kind}>
                  {t(`kind_${kind}`)}
                </option>
              ))}
              {!adult && <option value="mixed">{t("mixed")}</option>}
            </Select>
          </Label>
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
                {t("rootSavedScanFailed")}
              </p>
              <ErrorNotice error={scan.error} />
            </div>
          )}
          <div className="flex flex-wrap gap-3">
            <Button
              disabled={working || picking || !form.enabled}
              onClick={() => void persist(true)}
            >
              {t(
                operation === "scan"
                  ? "rootSavingAndScanning"
                  : scan.isError && savedVersion
                    ? "rootRetryScan"
                    : "rootSaveAndScan",
              )}
            </Button>
            <Button
              type="submit"
              variant="outline"
              disabled={working || picking}
            >
              {t(operation === "save" ? "saving" : "save")}
            </Button>
            <Button variant="ghost" disabled={working} onClick={closeEditor}>
              {t("cancel")}
            </Button>
          </div>
        </form>
      )}
      {completed && (
        <div
          className="notice mb-6 flex flex-wrap items-center justify-between gap-3"
          role="status"
        >
          <p>{t(completed.task ? "scanStarted" : "rootSaved")}</p>
          {completed.task && !adult && (
            <Button variant="outline" asChild>
              <Link
                to="/scanner"
                search={{ root: completed.root.id, task: completed.task.id }}
              >
                {t("rootOpenScan")}
              </Link>
            </Button>
          )}
          {completed.task && adult && onScan && (
            <Button
              variant="outline"
              onClick={() => onScan(completed.root.id, completed.task?.id)}
            >
              {t("rootOpenScan")}
            </Button>
          )}
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
        <div className="space-y-4">
          {roots.data.map((root) => (
            <article className="panel" key={root.id}>
              <div className="flex flex-wrap justify-between gap-4">
                <div>
                  <h2 className="font-semibold">
                    {root.name}{" "}
                    <span className="badge">
                      {t(root.enabled ? "enabled" : "disabled")}
                    </span>
                  </h2>
                  <p className="mt-2 break-all text-sm">{root.path}</p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {root.kind ? t(`kind_${root.kind}`) : t("mixed")} ·{" "}
                    {t("lastScan")}:{" "}
                    {root.last_scan_at
                      ? date(root.last_scan_at)
                      : t("neverScanned")}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={working}
                    onClick={() => edit(root)}
                  >
                    {t("edit")}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={(event) => {
                      removeTrigger.current = event.currentTarget;
                      deletion.reset();
                      setRemove(root);
                    }}
                  >
                    {t("remove")}
                  </Button>
                  {adult && onScan ? (
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={!root.enabled || working}
                      onClick={() => onScan(root.id)}
                    >
                      {t("scanRoot")}
                    </Button>
                  ) : !adult && root.enabled ? (
                    <Button asChild variant="ghost">
                      <Link
                        className="action-link"
                        to="/scanner"
                        search={{ root: root.id }}
                      >
                        {t("scanRoot")}
                      </Link>
                    </Button>
                  ) : null}
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </>
  );
}

export { SettingsPage } from "../settings/page";
