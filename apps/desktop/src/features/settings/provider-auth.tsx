import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { RootView } from "../../api/library";
import type { AuthMethod, AuthStorage } from "../../api/provider-auth";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { usePrivacy } from "../library/context";
import { Empty, ErrorNotice, Loading } from "../library/shared";
import {
  type ProviderSetupSearch,
  type SetupComplete,
  suggestedProvider,
} from "./setup-search";
import { SourceSetupActions } from "./source-setup-actions";
import { useProviderAuth } from "./use-provider-auth";
import { useProviderSetup, useSetupRoot } from "./use-provider-setup";
import "./auth-translations";
import "./setup-translations";

type EditorProps = {
  providerId: string;
  setup: ProviderSetupSearch;
  root?: RootView;
  onSetupComplete?: SetupComplete;
};
function PublicSourceEditor({
  providerId,
  setup,
  root,
  onSetupComplete,
}: EditorProps) {
  const { t } = useTranslation();
  const workflow = useProviderSetup(providerId, setup, root, onSetupComplete);
  return (
    <>
      <p className="notice mt-4">{t("setupPublicProvider")}</p>
      <SourceSetupActions
        workflow={workflow}
        setup={setup}
        credentialReady={Boolean(
          workflow.provider &&
            !workflow.provider.descriptor.requires_api_key &&
            !workflow.provider.descriptor.requires_login,
        )}
      />
    </>
  );
}
function AuthEditor({ providerId, setup, root, onSetupComplete }: EditorProps) {
  const { t } = useTranslation();
  const workflow = useProviderSetup(providerId, setup, root, onSetupComplete);
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
    saveCredential,
    clear,
  } = useProviderAuth(providerId);
  const working = pending !== null || workflow.phase !== null;
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
    return (
      <>
        <p className="notice mt-4">{t("providerAuthPublic")}</p>
        <SourceSetupActions workflow={workflow} setup={setup} credentialReady />
      </>
    );
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
        disabled={working || status.isFetching}
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
        onSubmit={(event) => {
          if (workflow.phase) event.preventDefault();
          else void save(event);
        }}
        noValidate
      >
        <Label className="filter-field">
          <span>{t("providerAuthMethod")}</span>
          <Select
            value={method}
            disabled={working}
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
            disabled={working}
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
            disabled={working}
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
              disabled={working}
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
              working ||
              !secret.trim() ||
              (method === "affiliate" && !affiliate.trim())
            }
          >
            {t(pending === "save" ? "providerAuthSaving" : "providerAuthSave")}
          </Button>
          <Button
            variant="outline"
            disabled={working || !status.data.configured}
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
      <SourceSetupActions
        workflow={workflow}
        setup={setup}
        credentialReady={status.data.configured}
        saveCredential={secret.trim() ? () => saveCredential(false) : undefined}
        disabled={
          pending !== null ||
          (Boolean(secret.trim()) &&
            method === "affiliate" &&
            !affiliate.trim())
        }
      />
      {outcome && (
        <p className="notice error-notice mt-4" role="alert">
          {t(
            outcome === "save_failed"
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
export function ProviderAuthSettings({
  setup = {},
  onSetupComplete,
}: {
  setup?: ProviderSetupSearch;
  onSetupComplete?: SetupComplete;
} = {}) {
  const { t } = useTranslation();
  const { roots, root, requested, allowed } = useSetupRoot(setup);
  if (requested && (!allowed || roots.isPending)) return <Loading />;
  if (requested && roots.error)
    return (
      <ErrorNotice error={roots.error} retry={() => void roots.refetch()} />
    );
  if (requested && !root?.enabled)
    return <Empty title={t("setupRootUnavailable")} />;
  return (
    <ProviderAuthForm
      key={`${setup.root ?? ""}:${setup.provider ?? ""}`}
      setup={setup}
      root={root}
      onSetupComplete={onSetupComplete}
    />
  );
}
function ProviderAuthForm({
  setup,
  root,
  onSetupComplete,
}: {
  setup: ProviderSetupSearch;
  root?: RootView;
  onSetupComplete?: SetupComplete;
}) {
  const { t } = useTranslation();
  const [provider, setProvider] = useState<string>(
    setup.provider ?? suggestedProvider(root?.kind ?? setup.kind),
  );
  const { includeAdult } = usePrivacy();
  const selectedProvider =
    !includeAdult && ["dmm", "wikidata"].includes(provider) ? "tmdb" : provider;
  return (
    <section className="panel" aria-labelledby="provider-auth-title">
      <h2 id="provider-auth-title" className="section-title">
        {t("providerAuthTitle")}
      </h2>
      {root && (
        <p className="mb-4 text-sm text-muted-foreground">
          {t("setupForRoot", { name: root.name })}
        </p>
      )}
      <Label className="filter-field">
        <span>{t("providerAuthProvider")}</span>
        <Select
          value={selectedProvider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="tmdb">TMDB</option>
          <option value="anilist">AniList</option>
          <option value="wikipedia.html">Wikipedia</option>
          <option value="musicbrainz">MusicBrainz</option>
          <option value="coverartarchive">Cover Art Archive</option>
          {includeAdult && (
            <>
              <option value="dmm">DMM / FANZA</option>
              <option value="wikidata">{t("providerAuthWikidata")}</option>
            </>
          )}
        </Select>
      </Label>
      {["musicbrainz", "coverartarchive"].includes(selectedProvider) ? (
        <PublicSourceEditor
          key={selectedProvider}
          providerId={selectedProvider}
          setup={setup}
          root={root}
          onSetupComplete={onSetupComplete}
        />
      ) : (
        <AuthEditor
          key={selectedProvider}
          providerId={selectedProvider}
          setup={setup}
          root={root}
          onSetupComplete={onSetupComplete}
        />
      )}
    </section>
  );
}
