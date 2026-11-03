import { Link } from "@tanstack/react-router";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { MediaKind } from "../../api/library";
import { Checkbox } from "../../components/ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { Textarea } from "../../components/ui/textarea";
import { useRootEditor } from "./use-root-editor";
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
}: {
  adult?: boolean;
  onScan?: (rootId: string) => void;
}) {
  const { t } = useTranslation();
  const { date } = useFormat();
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
  } = useRootEditor(adult);
  return (
    <>
      <PageHeading title={t("roots")}>
        <Button ref={addTrigger} onClick={() => edit()}>
          {t("addRoot")}
        </Button>
      </PageHeading>
      {form && (
        <form className="panel mb-6 space-y-4" onSubmit={submit} noValidate>
          <h2 className="section-title">{t(form.id ? "edit" : "addRoot")}</h2>
          <Label className="filter-field">
            <span>{t("rootName")}</span>
            <Input
              value={form.name}
              onChange={(event) =>
                setForm({ ...form, name: event.target.value })
              }
              required
            />
          </Label>
          <Label className="filter-field">
            <span>{t("rootPath")}</span>
            <Input
              value={form.path}
              onChange={(event) =>
                setForm({ ...form, path: event.target.value })
              }
              required
            />
          </Label>
          <Label className="filter-field">
            <span>{t("rootType")}</span>
            <Select
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
          <Label className="filter-field">
            <span>{t("excludePatterns")}</span>
            <Textarea
              value={excludes}
              onChange={(event) => setExcludes(event.target.value)}
              placeholder={t("excludeHelp")}
              rows={3}
            />
          </Label>
          <Label className="flex gap-2">
            <Checkbox
              checked={form.enabled}
              onChange={(event) =>
                setForm({ ...form, enabled: event.target.checked })
              }
            />
            {t("enabled")}
          </Label>
          {validation && <p role="alert">{t("invalidRoot")}</p>}
          {save.isError && <ErrorNotice error={save.error} />}
          <div className="flex gap-3">
            <Button type="submit" disabled={save.isPending}>
              {t(save.isPending ? "saving" : "save")}
            </Button>
            <Button variant="outline" onClick={() => setForm(null)}>
              {t("cancel")}
            </Button>
          </div>
        </form>
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
                  {adult ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => onScan?.(root.id)}
                    >
                      {t("scanRoot")}
                    </Button>
                  ) : (
                    <Button asChild variant="ghost">
                      <Link
                        className="action-link"
                        to="/scanner"
                        search={{ root: root.id }}
                      >
                        {t("scanRoot")}
                      </Link>
                    </Button>
                  )}
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
