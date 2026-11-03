import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DesktopError } from "../../api/desktop";
import {
  type AuthMethod,
  type AuthStorage,
  providerAuthApi,
} from "../../api/provider-auth";
import { Button } from "../../components/ui/button";
import { ErrorNotice } from "../library/shared";
import "./auth-translations";

function AuthEditor({ providerId }: { providerId: string }) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const active = useRef(false);
  const [method, setMethod] = useState<AuthMethod>("bearer");
  const [storage, setStorage] = useState<AuthStorage>("system");
  const [secret, setSecret] = useState("");
  const [pending, setPending] = useState<"save" | "clear" | null>(null);
  const [outcome, setOutcome] = useState<
    "saved" | "cleared" | "save_failed" | "clear_failed" | null
  >(null);
  const [error, setError] = useState<unknown>();
  const [invalid, setInvalid] = useState(false);
  const status = useQuery({
    queryKey: ["provider-auth", providerId],
    queryFn: () => providerAuthApi.status(providerId),
    retry: false,
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    if (status.data) {
      setMethod(
        status.data.supported_methods.includes(status.data.method)
          ? status.data.method
          : status.data.supported_methods[0],
      );
    }
  }, [status.data]);
  const needsSecret = status.data?.supported_methods.some(
    (value) => value !== "none",
  );
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const submitted = secret.trim();
    const valid =
      submitted.length > 0 &&
      !/\s/.test(submitted) &&
      [...submitted].every(
        (character) =>
          character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127,
      );
    setInvalid(!valid);
    if (!valid) {
      setSecret("");
      return;
    }
    setSecret("");
    setPending("save");
    setOutcome(null);
    setError(undefined);
    try {
      const saved = await providerAuthApi.save({
        provider_id: providerId,
        method,
        storage,
        secret: submitted,
      });
      if (!saved.configured) throw new DesktopError("invalid_response");
      if (active.current) {
        client.setQueryData(["provider-auth", providerId], saved);
        setOutcome("saved");
      }
    } catch (failure: unknown) {
      if (active.current) {
        setError(failure);
        setOutcome("save_failed");
      }
    } finally {
      if (active.current) {
        setSecret("");
        setPending(null);
      }
    }
  }
  async function clear() {
    if (pending) return;
    setSecret("");
    setPending("clear");
    setOutcome(null);
    setError(undefined);
    setInvalid(false);
    try {
      const cleared = await providerAuthApi.clear(providerId);
      if (cleared.configured) throw new DesktopError("invalid_response");
      if (active.current) {
        client.setQueryData(["provider-auth", providerId], cleared);
        setOutcome("cleared");
      }
    } catch (failure: unknown) {
      if (active.current) {
        setError(failure);
        setOutcome("clear_failed");
      }
    } finally {
      if (active.current) {
        setSecret("");
        setPending(null);
      }
    }
  }
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
        <label className="filter-field">
          <span>{t("providerAuthMethod")}</span>
          <select
            value={method}
            disabled={pending !== null}
            onChange={(event) => {
              setMethod(event.target.value as AuthMethod);
              setSecret("");
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
                    value === "bearer"
                      ? "providerAuthBearer"
                      : "providerAuthApiKey",
                  )}
                </option>
              ))}
          </select>
        </label>
        <p className="text-sm text-muted-foreground">
          {t(
            method === "api_key"
              ? "providerAuthApiKeyHelp"
              : "providerAuthBearerHelp",
          )}
        </p>
        <label className="filter-field">
          <span>{t("providerAuthStorage")}</span>
          <select
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
          </select>
        </label>
        <p className="text-sm text-muted-foreground">
          {t(
            storage === "system"
              ? "providerAuthSystemHelp"
              : "providerAuthSessionHelp",
          )}
        </p>
        <label className="filter-field">
          <span>{t("providerAuthSecret")}</span>
          <input
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
        </label>
        <p className="text-sm text-muted-foreground">
          {t("providerAuthNoEcho")}
        </p>
        {invalid && (
          <p className="notice error-notice" role="alert">
            {t("providerAuthInvalid")}
          </p>
        )}
        <div className="flex flex-wrap gap-3">
          <Button type="submit" disabled={pending !== null || !secret.trim()}>
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
  return (
    <section className="panel" aria-labelledby="provider-auth-title">
      <h2 id="provider-auth-title" className="section-title">
        {t("providerAuthTitle")}
      </h2>
      <p className="mb-4 text-sm text-muted-foreground">
        {t("providerAuthHelp")}
      </p>
      <label className="filter-field">
        <span>{t("providerAuthProvider")}</span>
        <select
          value={provider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="tmdb">TMDB</option>
          <option value="anilist">AniList</option>
          <option value="wikipedia.html">Wikipedia</option>
        </select>
      </label>
      <AuthEditor key={provider} providerId={provider} />
    </section>
  );
}
