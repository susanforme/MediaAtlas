import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AuthMethod, AuthStorage } from "../../api/provider-auth";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { usePrivacy } from "../library/context";
import { ErrorNotice } from "../library/shared";
import { useProviderAuth } from "./use-provider-auth";
import "./auth-translations";

function AuthEditor({ providerId }: { providerId: string }) {
  const { t } = useTranslation();
  const {
    method,
    setMethod,
    storage,
    setStorage,
    secret,
    setSecret,
    affiliate,
    setAffiliate,
    pending,
    outcome,
    setOutcome,
    error,
    setError,
    invalid,
    setInvalid,
    status,
    needsSecret,
    save,
    clear,
  } = useProviderAuth(providerId);
  if (status.isPending)
    return (
      <p role="status" className="mt-4 text-sm text-muted-foreground">
        {t("providerAuthLoading")}
      </p>
    );
  if (status.error)
    return (
      <div className="mt-4">
        <ErrorNotice error={status.error} retry={() => void status.refetch()} />
      </div>
    );
  if (!needsSecret)
    return <p className="notice mt-4">{t("providerAuthPublic")}</p>;
  return (
    <div className="mt-4">
      <p className="badge" role="status">
        {status.data.configured
          ? t("providerAuthConfigured")
          : t("providerAuthMissing")}
      </p>
      {!status.data.configured &&
        status.data.status === "credentials_unavailable" && (
          <p className="notice mt-3">
            {t("providerFailure_credentials_unavailable", {
              defaultValue: t("providerAuthSaveFailed"),
            })}
          </p>
        )}
      {status.data.configured && (
        <p className="mt-2 text-sm text-muted-foreground">
          {t("providerAuthStorageStatus", {
            storage: t(
              status.data.storage === "system"
                ? "providerAuthSystem"
                : "providerAuthSession",
            ),
          })}
        </p>
      )}
      {status.data.status === "cleanup_pending" && (
        <p className="notice mt-3" role="status">
          {t(
            status.data.configured
              ? "providerAuthCleanupPending"
              : "providerAuthCleanupDisabled",
          )}
        </p>
      )}
      <Button
        variant="outline"
        className="mt-3"
        disabled={pending !== null || status.isFetching}
        onClick={() => {
          setSecret("");
          setAffiliate("");
          setInvalid(false);
          setOutcome(null);
          setError(undefined);
          void status.refetch();
        }}
      >
        {t("providerAuthRefresh")}
      </Button>
      <form
        className="mt-4 space-y-4"
        onSubmit={(event) => void save(event)}
        noValidate
      >
        <Label className="filter-field">
          <span>{t("providerAuthMethod")}</span>
          <Select
            value={method}
            disabled={pending !== null}
            onChange={(event) => {
              setMethod(event.target.value as AuthMethod);
              setSecret("");
              setAffiliate("");
              setOutcome(null);
              setError(undefined);
              setInvalid(false);
            }}
          >
            {status.data.supported_methods
              .filter((value) => value !== "none")
              .map((value) => (
                <option key={value} value={value}>
                  {t(
                    value === "affiliate"
                      ? "providerAuthAffiliate"
                      : value === "bearer"
                        ? "providerAuthBearer"
                        : "providerAuthApiKey",
                  )}
                </option>
              ))}
          </Select>
        </Label>
        <Label className="filter-field">
          <span>{t("providerAuthStorage")}</span>
          <Select
            value={storage}
            disabled={pending !== null}
            onChange={(event) => {
              setStorage(event.target.value as AuthStorage);
              setOutcome(null);
              setError(undefined);
            }}
          >
            <option value="system">{t("providerAuthSystem")}</option>
            <option value="session_only">{t("providerAuthSession")}</option>
          </Select>
        </Label>
        <Label className="filter-field">
          <span>
            {t(
              method === "affiliate"
                ? "providerAuthApiId"
                : "providerAuthSecret",
            )}
          </span>
          <Input
            type="password"
            name="provider-credential"
            autoComplete="off"
            spellCheck={false}
            value={secret}
            maxLength={4096}
            disabled={pending !== null}
            onChange={(event) => {
              setSecret(event.target.value);
              setInvalid(false);
            }}
          />
        </Label>
        {method === "affiliate" && (
          <Label className="filter-field">
            <span>{t("providerAuthAffiliateId")}</span>
            <Input
              type="password"
              name="provider-affiliate"
              autoComplete="off"
              spellCheck={false}
              maxLength={4096}
              value={affiliate}
              disabled={pending !== null}
              onChange={(event) => {
                setAffiliate(event.target.value);
                setInvalid(false);
              }}
            />
          </Label>
        )}
        {status.data.configured && (
          <p className="text-sm text-muted-foreground">
            {t("providerAuthNoEcho")}
          </p>
        )}
        {invalid && (
          <p className="notice error-notice" role="alert">
            {t("providerAuthInvalid")}
          </p>
        )}
        <div className="flex flex-wrap gap-3">
          <Button
            type="submit"
            disabled={
              pending !== null ||
              !secret.trim() ||
              (method === "affiliate" && !affiliate.trim())
            }
          >
            {t(pending === "save" ? "providerAuthSaving" : "providerAuthSave")}
          </Button>
          <Button
            variant="outline"
            disabled={pending !== null || !status.data.configured}
            onClick={() => void clear()}
          >
            {t(
              pending === "clear"
                ? "providerAuthDeleting"
                : "providerAuthClear",
            )}
          </Button>
        </div>
      </form>
      {outcome &&
        !(
          ["saved", "cleared"].includes(outcome) &&
          status.data.status === "cleanup_pending"
        ) && (
          <p
            className={`notice mt-4 ${outcome.endsWith("failed") ? "error-notice" : ""}`}
            role={outcome.endsWith("failed") ? "alert" : "status"}
          >
            {t(
              outcome === "saved"
                ? "providerAuthSaved"
                : outcome === "cleared"
                  ? "providerAuthCleared"
                  : outcome === "save_failed"
                    ? "providerAuthSaveFailed"
                    : "providerAuthClearFailed",
            )}
          </p>
        )}
      {error !== undefined && (
        <div className="mt-3">
          <ErrorNotice error={error} />
        </div>
      )}
    </div>
  );
}
export function ProviderAuthSettings() {
  const { t } = useTranslation();
  const [provider, setProvider] = useState("tmdb");
  const { includeAdult } = usePrivacy();
  const selectedProvider =
    !includeAdult && ["dmm", "wikidata"].includes(provider) ? "tmdb" : provider;
  return (
    <section className="panel" aria-labelledby="provider-auth-title">
      <h2 id="provider-auth-title" className="section-title">
        {t("providerAuthTitle")}
      </h2>
      <Label className="filter-field">
        <span>{t("providerAuthProvider")}</span>
        <Select
          value={selectedProvider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="tmdb">TMDB</option>
          <option value="anilist">AniList</option>
          <option value="wikipedia.html">Wikipedia</option>
          {includeAdult && (
            <>
              <option value="dmm">DMM / FANZA</option>
              <option value="wikidata">{t("providerAuthWikidata")}</option>
            </>
          )}
        </Select>
      </Label>
      <AuthEditor key={selectedProvider} providerId={selectedProvider} />
    </section>
  );
}
