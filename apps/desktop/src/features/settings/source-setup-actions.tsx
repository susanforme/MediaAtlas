import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { ErrorNotice } from "../library/shared";
import type { ProviderSetupSearch } from "./setup-search";
import type { useProviderSetup } from "./use-provider-setup";
import "./setup-translations";

export function SourceSetupActions({
  workflow,
  setup,
  credentialReady,
  saveCredential,
  disabled = false,
}: {
  workflow: ReturnType<typeof useProviderSetup>;
  setup: ProviderSetupSearch;
  credentialReady: boolean;
  saveCredential?: () => Promise<boolean>;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const action =
    workflow.phase === "save"
      ? "setupSaving"
      : workflow.phase === "enable"
        ? "setupEnabling"
        : workflow.phase === "start"
          ? "setupStarting"
          : saveCredential
            ? setup.from === "library" && setup.root
              ? "setupSaveEnableContinue"
              : setup.from === "scraper"
                ? "setupSaveEnableReturn"
                : "setupSaveEnable"
            : setup.from === "library" && setup.root
              ? workflow.provider?.config.enabled
                ? "setupContinue"
                : "setupEnableContinue"
              : setup.from === "scraper"
                ? "setupEnableReturn"
                : "setupEnable";
  return (
    <div className="mt-4 space-y-3 border-t border-border pt-4">
      {workflow.snapshot.isPending ? (
        <p role="status">{t("setupLoading")}</p>
      ) : workflow.snapshot.error ? (
        <ErrorNotice
          error={workflow.snapshot.error}
          retry={() => void workflow.snapshot.refetch()}
        />
      ) : workflow.unavailable ? (
        <div className="space-y-3">
          <p className="notice" role="alert">
            {t(workflow.incompatible ? "setupWrongType" : "setupUnavailable")}
          </p>
          <Button
            variant="outline"
            disabled={workflow.snapshot.isFetching}
            onClick={() => void workflow.snapshot.refetch()}
          >
            {t("setupRetry")}
          </Button>
        </div>
      ) : (
        <>
          <p className="text-sm text-muted-foreground">
            {t(
              workflow.provider?.config.enabled
                ? "setupProviderEnabled"
                : "setupProviderDisabled",
            )}
          </p>
          <Button
            disabled={
              disabled ||
              workflow.phase !== null ||
              (!credentialReady && !saveCredential)
            }
            onClick={() => void workflow.run(credentialReady, saveCredential)}
          >
            {t(action)}
          </Button>
        </>
      )}
      {workflow.failurePhase && (
        <p role="alert" className="notice error-notice">
          {t(
            workflow.failurePhase === "start"
              ? "setupStartFailed"
              : "setupEnableFailed",
          )}
        </p>
      )}
      {workflow.error !== undefined && <ErrorNotice error={workflow.error} />}
    </div>
  );
}
