import { useTranslation } from "react-i18next";
import { databaseMaintenanceApi as api } from "../../api/maintenance";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import { useMaintenance } from "./use-maintenance";
import "./translations";

export function MaintenancePage({
  embedded = false,
}: {
  embedded?: boolean;
} = {}) {
  const { t } = useTranslation();
  const {
    working,
    preview,
    setPreview,
    error,
    setError,
    message,
    exportPath,
    status,
    makePreview,
    confirm,
    exportDiagnostics,
  } = useMaintenance();
  return (
    <>
      {!embedded && <PageHeading title={t("maintenanceTitle")} />}
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
              <Label className="filter-field">
                <span>{t("maintenanceDirectory")}</span>
                <Input readOnly value={status.data.directory} />
              </Label>
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
          <div className="flex flex-wrap gap-3">
            <Button disabled={working} onClick={() => void exportDiagnostics()}>
              {t("maintenanceDiagnostics")}
            </Button>
            <Button
              variant="outline"
              disabled={working}
              onClick={() => void makePreview("cache")}
            >
              {t("maintenanceCachePreview")}
            </Button>
          </div>
          {exportPath && (
            <Input
              aria-label={t("maintenanceExported")}
              readOnly
              value={exportPath}
              className="w-full"
            />
          )}
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
