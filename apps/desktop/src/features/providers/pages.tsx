/** biome-ignore-all lint/suspicious/noArrayIndexKey: Diagnostic rows have no local state; multiple requests can share one origin and attempt. */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type ProviderConfig,
  type ProviderView,
  providerApi,
} from "../../api/providers";
import { Button } from "../../components/ui/button";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  useFormat,
} from "../library/shared";

function useSnapshot() {
  return useQuery({
    queryKey: ["providers"],
    queryFn: providerApi.snapshot,
    refetchInterval: 2000,
  });
}
const numericFields = [
  ["timeout_ms", "providerTimeout", 1, 120000],
  ["concurrency", "providerConcurrency", 1, 32],
  ["image_concurrency", "providerImageConcurrency", 1, 16],
  ["interval_ms", "providerInterval", 0, 60000],
  ["max_response_bytes", "providerMaxResponse", 1, 16777216],
  ["max_retries", "providerRetries", 0, 3],
] as const;
function ProviderConfiguration({ provider }: { provider: ProviderView }) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [value, setValue] = useState<ProviderConfig>(provider.config);
  const [invalid, setInvalid] = useState(false);
  useEffect(() => {
    setValue(provider.config);
  }, [provider.config]);
  const save = useMutation({
    mutationFn: () => providerApi.updateConfig(provider.descriptor.id, value),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["providers"] });
    },
  });
  return (
    <details className="mt-5">
      <summary>{t("providerConfig")}</summary>
      <form
        className="mt-4"
        onSubmit={(event) => {
          event.preventDefault();
          const reference = value.credential_ref;
          const validReference =
            reference === null || /^[A-Za-z0-9._-]{1,128}$/.test(reference);
          const valid =
            validReference &&
            numericFields.every(
              ([key, , min, max]) =>
                Number.isSafeInteger(value[key]) &&
                value[key] >= min &&
                value[key] <= max,
            );
          setInvalid(!valid);
          if (valid) save.mutate();
        }}
        noValidate
      >
        <div className="grid gap-3 sm:grid-cols-2">
          {numericFields.map(([key, label, min, max]) => (
            <label className="filter-field" key={key}>
              <span>{t(label)}</span>
              <input
                type="number"
                min={min}
                max={max}
                value={Number.isNaN(value[key]) ? "" : value[key]}
                onChange={(event) =>
                  setValue((old) => ({
                    ...old,
                    [key]:
                      event.target.value === ""
                        ? Number.NaN
                        : Number(event.target.value),
                  }))
                }
              />
            </label>
          ))}
        </div>
        <label className="filter-field mt-4">
          <span>{t("providerCredentialRef")}</span>
          <input
            maxLength={128}
            autoComplete="off"
            value={value.credential_ref ?? ""}
            readOnly={!provider.descriptor.fixture}
            onChange={(event) =>
              setValue((old) => ({
                ...old,
                credential_ref: event.target.value || null,
              }))
            }
          />
        </label>
        {!provider.descriptor.fixture && (
          <p className="mt-2 text-sm text-muted-foreground">
            {t("providerAuthSettingsHint")}
          </p>
        )}
        <Button className="mt-4" type="submit" disabled={save.isPending}>
          {t("providerSaveConfig")}
        </Button>
        {invalid && <p role="alert">{t("providerConfigInvalid")}</p>}
        {save.isError && <ErrorNotice error={save.error} />}{" "}
        {save.isSuccess && <p role="status">{t("providerConfigSaved")}</p>}
      </form>
    </details>
  );
}
function ProviderCard({ provider }: { provider: ProviderView }) {
  const { t, i18n } = useTranslation();
  const { number } = useFormat();
  const client = useQueryClient();
  const [query, setQuery] = useState("");
  const refresh = () => {
    void client.invalidateQueries({ queryKey: ["providers"] });
  };
  const enabled = useMutation({
    mutationFn: (value: boolean) =>
      providerApi.setEnabled(provider.descriptor.id, value),
    onSuccess: refresh,
  });
  const start = useMutation({
    mutationFn: () =>
      providerApi.start(
        provider.descriptor.id,
        query.trim(),
        i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN",
      ),
    onSuccess: refresh,
  });
  return (
    <article className="panel">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="section-title">
          {provider.descriptor.fixture
            ? t("providerFixtureName")
            : provider.descriptor.name}
        </h2>
        <span className="badge">
          {t(`providerHealth_${provider.health}`, {
            defaultValue: t("providerHealthUnknown"),
          })}
        </span>
      </div>
      {provider.descriptor.fixture && (
        <p className="notice mb-4">{t("providerFixtureNotice")}</p>
      )}
      <p className="mb-3 text-sm">
        {t("providerCapabilities")}:{" "}
        {provider.descriptor.capabilities
          .map((cap) => t(`providerCapability_${cap}`, { defaultValue: cap }))
          .join(" · ")}
      </p>
      <p className="mb-4 text-xs text-muted-foreground">
        {t("providerRevision")}: {number(provider.config.revision)} ·{" "}
        {t("providerTimeout")}: {number(provider.config.timeout_ms)} ms ·{" "}
        {t("providerConcurrency")}: {number(provider.config.concurrency)}
      </p>
      <label className="mb-4 flex items-center gap-2">
        <input
          type="checkbox"
          checked={provider.config.enabled}
          disabled={enabled.isPending}
          onChange={(event) => enabled.mutate(event.target.checked)}
        />
        {t("providerEnabled")}
      </label>
      {provider.descriptor.fixture ? (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (query.trim() && provider.config.enabled) start.mutate();
          }}
          className="flex flex-wrap items-end gap-3"
        >
          <label className="filter-field flex-1">
            <span>{t("providerQuery")}</span>
            <input
              maxLength={200}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              autoComplete="off"
            />
          </label>
          <Button
            type="submit"
            disabled={
              !query.trim() || !provider.config.enabled || start.isPending
            }
          >
            {t("providerRun")}
          </Button>
        </form>
      ) : (
        <div className="flex flex-wrap gap-3">
          <Link to="/scraper" className="action-link">
            {t("providerOpenScraper")}
          </Link>
          <Link to="/settings" className="action-link">
            {t("providerOpenAuthSettings")}
          </Link>
        </div>
      )}
      {provider.descriptor.fixture && start.isSuccess && (
        <p role="status" className="mt-3">
          {t("providerJobStarted")}
        </p>
      )}
      <ProviderConfiguration provider={provider} />
      <details className="mt-4">
        <summary>{t("providerDetails")}</summary>
        <dl className="mt-3 space-y-2 text-sm">
          <div>
            <dt>{t("providerLicense")}</dt>
            <dd>{provider.descriptor.license}</dd>
          </div>
          <div>
            <dt>{t("providerPolicy")}</dt>
            <dd>{provider.descriptor.rate_limit_policy}</dd>
          </div>
          <div>
            <dt>{t("providerAttribution")}</dt>
            <dd>{provider.descriptor.attribution}</dd>
          </div>
          <div>
            <dt>{t("providerDataUsage")}</dt>
            <dd>{provider.descriptor.data_usage_notes}</dd>
          </div>
          <div>
            <dt>{t("providerApiKey")}</dt>
            <dd>
              {t(
                provider.descriptor.requires_api_key
                  ? "providerYes"
                  : "providerNo",
              )}
            </dd>
          </div>
          <div>
            <dt>{t("providerLogin")}</dt>
            <dd>
              {t(
                provider.descriptor.requires_login
                  ? "providerYes"
                  : "providerNo",
              )}
            </dd>
          </div>
        </dl>
        <div className="mt-3 flex flex-wrap gap-3">
          {(
            [
              ["providerDocumentation", provider.descriptor.documentation_url],
              ["providerTerms", provider.descriptor.terms_url],
              ["providerPrivacy", provider.descriptor.privacy_url],
            ] as const
          ).map(
            ([label, url]) =>
              url && (
                <a key={label} href={url} target="_blank" rel="noreferrer">
                  {t(label)}
                </a>
              ),
          )}
        </div>
      </details>
      {enabled.isError && <ErrorNotice error={enabled.error} />}
      {provider.descriptor.fixture && start.isError && (
        <ErrorNotice error={start.error} />
      )}
    </article>
  );
}
export function ProvidersPage() {
  const { t } = useTranslation();
  const snapshot = useSnapshot();
  return (
    <>
      <PageHeading
        title={t("providers")}
        description={t("providersDescription")}
      />
      <p className="notice mb-5">{t("providerCredentialsNotice")}</p>
      {snapshot.isPending ? (
        <Loading />
      ) : snapshot.isError ? (
        <ErrorNotice
          error={snapshot.error}
          retry={() => void snapshot.refetch()}
        />
      ) : snapshot.data.providers.length ? (
        <div className="space-y-4">
          {snapshot.data.providers.map((provider) => (
            <ProviderCard key={provider.descriptor.id} provider={provider} />
          ))}
        </div>
      ) : (
        <Empty title={t("noProviders")} />
      )}
    </>
  );
}
export function ProviderTasks() {
  const { t } = useTranslation();
  const { date } = useFormat();
  const client = useQueryClient();
  const snapshot = useSnapshot();
  const cancel = useMutation({
    mutationFn: providerApi.cancel,
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["providers"] });
    },
  });
  return (
    <section className="mt-8" aria-label={t("providerTasks")}>
      <h2 className="section-title">{t("providerTasks")}</h2>
      {cancel.isError && <ErrorNotice error={cancel.error} />}{" "}
      {snapshot.isPending ? (
        <Loading />
      ) : snapshot.isError ? (
        <ErrorNotice
          error={snapshot.error}
          retry={() => void snapshot.refetch()}
        />
      ) : !snapshot.data.tasks.length ? (
        <Empty title={t("noProviderTasks")} />
      ) : (
        <div className="space-y-4">
          {snapshot.data.tasks.map((task) => (
            <article className="panel" key={task.id}>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <span className="badge">{t(`state_${task.state}`)}</span>
                {["queued", "running", "waiting_for_user"].includes(
                  task.state,
                ) && (
                  <Button
                    variant="outline"
                    disabled={cancel.isPending}
                    onClick={() => cancel.mutate(task.id)}
                  >
                    {t("cancelTask")}
                  </Button>
                )}
              </div>
              <p className="mt-3 break-all text-sm">{task.query}</p>
              <p className="mt-2 break-all text-xs text-muted-foreground">
                {task.provider_id} · {task.id} · {date(task.updated_at)}
              </p>
              {task.failure && (
                <p role="status" className="mt-3 text-sm">
                  {t(`providerFailure_${task.failure}`)}
                </p>
              )}
              {task.result_summary && (
                <p className="mt-2 text-sm">
                  {t(`providerResult_${task.result_summary}`, {
                    defaultValue: t("providerResultReady"),
                  })}
                </p>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
export function NetworkInspectorPage() {
  const { t } = useTranslation();
  const { number } = useFormat();
  const snapshot = useSnapshot();
  return (
    <>
      <PageHeading
        title={t("networkInspector")}
        description={t("networkInspectorDescription")}
      />
      {snapshot.isPending ? (
        <Loading />
      ) : snapshot.isError ? (
        <ErrorNotice
          error={snapshot.error}
          retry={() => void snapshot.refetch()}
        />
      ) : !snapshot.data.requests.length ? (
        <Empty title={t("noNetworkRequests")} />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                {[
                  "providerLabel",
                  "providerRequest",
                  "providerDuration",
                  "providerStatus",
                  "providerAttempt",
                ].map((key) => (
                  <th key={key}>{t(key)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {snapshot.data.requests.map((record, index) => (
                <tr
                  key={`${record.task_id}-${record.engine}-${record.attempt}-${record.method}-${record.safe_url}-${index}`}
                >
                  <td className="break-all text-xs">
                    {record.provider_id}
                    <p>{record.task_id}</p>
                  </td>
                  <td className="break-all text-xs">
                    {record.engine} · {record.method}
                    <p>
                      {record.safe_url === "[redacted]"
                        ? t("providerRedacted")
                        : record.safe_url}
                    </p>
                    {record.summary && (
                      <p>
                        {t(`providerSummary_${record.summary}`, {
                          defaultValue: t("providerSummaryRequest"),
                        })}
                      </p>
                    )}
                  </td>
                  <td>{number(record.duration_ms)} ms</td>
                  <td>
                    {record.status === null
                      ? t("providerNoStatus")
                      : number(record.status)}
                    {record.failure && (
                      <p>{t(`providerFailure_${record.failure}`)}</p>
                    )}
                  </td>
                  <td>{number(record.attempt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
