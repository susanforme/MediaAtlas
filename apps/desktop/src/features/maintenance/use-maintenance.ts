import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  databaseMaintenanceApi as api,
  type BackupPreview,
  type CachePreview,
  type RestorePreview,
} from "../../api/maintenance";
import { usePrivacy } from "../library/context";

type Preview =
  | { kind: "backup"; value: BackupPreview }
  | { kind: "restore"; value: RestorePreview }
  | { kind: "cache"; value: CachePreview };
export function useMaintenance() {
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
    const epoch = generation.current;
    try {
      await action();
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
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
  async function exportDiagnostics() {
    const epoch = generation.current;
    await run(async () => {
      const exported = await api.diagnostics();
      if (epoch === generation.current) {
        setExportPath(exported.path);
        setMessage(t("maintenanceExported"));
      }
    });
  }
  return {
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
  };
}
