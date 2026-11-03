import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  databaseMaintenanceApi as api,
  type BackupPreview,
  type CachePreview,
  type RestorePreview,
} from "../../api/maintenance";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "../library/context";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import "./translations";

type Preview =
  | { kind: "backup"; value: BackupPreview }
  | { kind: "restore"; value: RestorePreview }
  | { kind: "cache"; value: CachePreview };
export function MaintenancePage() {
  const { t } = useTranslation();
  const { includeAdult } = usePrivacy();
  const generation = useRef(0);
  const busy = useRef(false);
  const [working, setWorking] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState<unknown>();
  const [message, setMessage] = useState("");
  const [exportPath, setExportPath] = useState("");
  const status = useQuery({
    queryKey: ["database-maintenance"],
    queryFn: api.status,
  });
  useEffect(() => {
    generation.current += 1;
    if (!includeAdult) setExportPath("");
    setPreview(null);
    setMessage("");
    return () => {
      generation.current += 1;
    };
  }, [includeAdult]);
  async function run(action: () => Promise<void>) {
    if (busy.current) return;
    busy.current = true;
    setWorking(true);
    setError(undefined);
    setMessage("");
    try {
      await action();
    } catch (failure) {
      setError(failure);
    } finally {
      busy.current = false;
      setWorking(false);
    }
  }
  async function makePreview(kind: Preview["kind"], id?: string) {
    const epoch = generation.current;
    setPreview(null);
    await run(async () => {
      const value =
        kind === "backup"
          ? await api.backupPreview()
          : kind === "restore"
            ? await api.restorePreview(id ?? "")
            : await api.cachePreview();
      if (epoch === generation.current) setPreview({ kind, value } as Preview);
    });
  }
  async function confirm() {
    const selected = preview;
    if (!selected) return;
    setPreview(null);
    const epoch = generation.current;
    await run(async () => {
      if (selected.kind === "backup") {
        await api.backupConfirm(selected.value.id);
        if (epoch === generation.current) setMessage(t("maintenanceSaved"));
      } else if (selected.kind === "restore") {
        await api.restoreConfirm(selected.value.id);
        if (epoch === generation.current) setMessage(t("maintenanceQueued"));
      } else {
        const result = await api.cacheConfirm(selected.value.id);
        if (epoch === generation.current)
          setMessage(
            t("maintenanceCacheCleared", { files: result.removed_files }),
          );
      }
      await status.refetch();
    });
  }
  return (
    <>
      <PageHeading title={t("maintenanceTitle")} />
      <div className="space-y-6">
        <section className="panel space-y-4">
          {status.isPending && <Loading />}
          {status.error && (
            <ErrorNotice
              error={status.error}
              retry={() => void status.refetch()}
            />
          )}
          {status.data && (
            <>
              <label className="filter-field">
                <span>{t("maintenanceDirectory")}</span>
                <input readOnly value={status.data.directory} />
              </label>
              {status.data.restore_pending && (
                <p role="status" className="notice">
                  {t("maintenancePending")}
                </p>
              )}
              <div className="flex flex-wrap gap-3">
                <Button
                  disabled={working || status.data.restore_pending}
                  onClick={() => void makePreview("backup")}
                >
                  {t("maintenanceBackupPreview")}
                </Button>
                <Button
                  variant="outline"
                  disabled={working}
                  onClick={() => void status.refetch()}
                >
                  {t("maintenanceRefresh")}
                </Button>
              </div>
              {!status.data.backups.length && (
                <p>{t("maintenanceNoBackups")}</p>
              )}
              <ul className="space-y-3">
                {status.data.backups.map((backup) => (
                  <li key={backup.id} className="panel">
                    <p>
                      {backup.created_at} · v{backup.app_version} ·{" "}
                      {backup.bytes} B
                    </p>
                    {backup.contains_private_metadata && (
                      <p className="badge">{t("maintenancePrivate")}</p>
                    )}
                    <Button
                      variant="outline"
                      disabled={working || status.data.restore_pending}
                      onClick={() => void makePreview("restore", backup.id)}
                    >
                      {t("maintenanceRestorePreview")}
                    </Button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
        {preview && (
          <section className="panel space-y-4" aria-label={t("preview")}>
            <p className="notice">
              {t(
                preview.kind === "backup"
                  ? "maintenanceBackupWarning"
                  : preview.kind === "restore"
                    ? "maintenanceRestoreWarning"
                    : "maintenanceCacheHelp",
              )}
            </p>
            {preview.kind === "cache" && (
              <p>
                {t("maintenanceCacheCount", {
                  files: preview.value.files,
                  bytes: preview.value.bytes,
                })}
              </p>
            )}
            {preview.kind !== "cache" &&
              preview.value.contains_private_metadata && (
                <p className="badge">{t("maintenancePrivate")}</p>
              )}
            <div className="flex flex-wrap gap-3">
              <Button disabled={working} onClick={() => void confirm()}>
                {t(
                  preview.kind === "backup"
                    ? "maintenanceBackupConfirm"
                    : preview.kind === "restore"
                      ? "maintenanceRestoreConfirm"
                      : "maintenanceCacheConfirm",
                )}
              </Button>
              <Button
                variant="outline"
                disabled={working}
                onClick={() => setPreview(null)}
              >
                {t("maintenanceDismiss")}
              </Button>
            </div>
          </section>
        )}
        <section className="panel space-y-4">
          <p>{t("maintenanceDiagnosticsHelp")}</p>
          <Button
            disabled={working}
            onClick={() =>
              void run(async () => {
                const exported = await api.diagnostics();
                setExportPath(exported.path);
                setMessage(t("maintenanceExported"));
              })
            }
          >
            {t("maintenanceDiagnostics")}
          </Button>
          {exportPath && (
            <input
              aria-label={t("maintenanceExported")}
              readOnly
              value={exportPath}
              className="w-full"
            />
          )}
          <Button
            variant="outline"
            disabled={working}
            onClick={() => void makePreview("cache")}
          >
            {t("maintenanceCachePreview")}
          </Button>
        </section>
        {working && (
          <div role="status">
            <p>{t("maintenanceWorking")}</p>
            <Button
              variant="outline"
              onClick={() => void api.cancel().catch(setError)}
            >
              {t("maintenanceCancel")}
            </Button>
          </div>
        )}
        {message && (
          <p role="status" className="notice">
            {message}
          </p>
        )}
        {error !== undefined && <ErrorNotice error={error} />}
      </div>
    </>
  );
}
