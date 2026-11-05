/** biome-ignore-all lint/suspicious/noArrayIndexKey: Diagnostic rows have no local state; multiple requests can share one origin and attempt. */
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "../../components/ui/table";
import "../scraper/translations";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import {
  type ProviderConfig,
  type ProviderView,
  providerApi,
} from "../../api/providers";
import { Anchor, Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { toast } from "../../components/ui/sonner";
import { usePrivacy } from "../library/context";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  useFormat,
} from "../library/shared";
import {
  type ProviderSetupSearch,
  SETUP_PROVIDERS,
  type SetupProvider,
} from "../settings/setup-search";

function useSnapshot(adult = false) {
  const { includeAdult } = usePrivacy();
  return useQuery({
    queryKey: adult ? ["adult", "providers"] : ["providers"],
    queryFn: adult ? adultApi.providers : providerApi.snapshot,
    enabled: !adult || includeAdult,
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
    mutationFn: () =>
      providerApi.updateConfig(provider.descriptor.id, {
        ...value,
        credential_ref: null,
      }),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["providers"] });
      toast.success(t("providerConfigSaved"));
    },
  });
  return (
    <Details className="mt-5">
      <DetailsSummary>{t("providerConfig")}</DetailsSummary>
      <form
        className="mt-4"
        onSubmit={(event) => {
          event.preventDefault();
          const valid = numericFields.every(
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
            <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]" key={key}>
              <span>{t(label)}</span>
              <Input
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
            </Label>
          ))}
        </div>
        <Button className="mt-4" type="submit" disabled={save.isPending}>
          {t("providerSaveConfig")}
        </Button>
        {invalid && <p role="alert">{t("providerConfigInvalid")}</p>}
        {save.isError && <ErrorNotice error={save.error} />}{" "}
      </form>
    </Details>
  );
}
function ProviderCard({
  provider,
  returnToScraper = false,
  returnMediaId,
  returnQueue,
  setup,
}: {
  provider: ProviderView;
  returnToScraper?: boolean;
  returnMediaId?: string;
  returnQueue?: string[];
  setup?: ProviderSetupSearch;
}) {
  const { t, i18n } = useTranslation();
  const scraperContext:
    | Pick<ProviderSetupSearch, "media" | "queue" | "session">
    | undefined =
    setup?.from === "scraper"
      ? setup
      : returnToScraper
        ? { media: returnMediaId, queue: returnQueue }
        : undefined;
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
    onSuccess: () => {
      refresh();
      toast.success(t("providerJobStarted"));
    },
  });
  return (
    <Card asChild>
      <article>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="mb-4 font-semibold">
            {provider.descriptor.fixture
              ? t("providerFixtureName")
              : provider.descriptor.name}
          </h2>
          <Badge variant="secondary">
            {t(`providerHealth_${provider.health}`, {
              defaultValue: t("providerHealthUnknown"),
            })}
          </Badge>
        </div>
        {provider.descriptor.fixture && (
          <Alert asChild>
            <p className="mb-4">{t("providerFixtureNotice")}</p>
          </Alert>
        )}
        <p className="mb-3 text-sm">
          {provider.descriptor.capabilities
            .map((cap) => t(`providerCapability_${cap}`, { defaultValue: cap }))
            .join(" · ")}
        </p>
        <Label className="mb-4 flex items-center gap-2">
          <Checkbox
            checked={provider.config.enabled}
            disabled={enabled.isPending}
            onChange={(event) => enabled.mutate(event.target.checked)}
          />
          {t("providerEnabled")}
        </Label>
        {provider.descriptor.fixture ? (
          <form
            onSubmit={(event) => {
              event.preventDefault();
              if (query.trim() && provider.config.enabled) start.mutate();
            }}
            className="flex flex-wrap items-end gap-3"
          >
            <Label className="flex min-w-0 flex-1 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
              <span>{t("providerQuery")}</span>
              <Input
                maxLength={200}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                autoComplete="off"
              />
            </Label>
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
            {setup?.from !== "library" && (
              <Button asChild variant="outline">
                <Link
                  to="/scraper"
                  search={{
                    media: scraperContext?.media,
                    queue: scraperContext?.queue,
                    session: scraperContext?.session,
                  }}
                >
                  {t("providerOpenScraper")}
                </Link>
              </Button>
            )}
            <Button asChild variant="outline">
              <Link
                to="/settings"
                search={{
                  ...(setup ??
                    (returnToScraper
                      ? {
                          from: "scraper",
                          media: returnMediaId,
                          queue: returnQueue,
                        }
                      : {})),
                  provider: SETUP_PROVIDERS.includes(
                    provider.descriptor.id as SetupProvider,
                  )
                    ? (provider.descriptor.id as SetupProvider)
                    : undefined,
                  tab: "auth",
                }}
              >
                {t("providerOpenAuthSettings")}
              </Link>
            </Button>
          </div>
        )}
        <ProviderConfiguration provider={provider} />
        <Details className="mt-4">
          <DetailsSummary>{t("providerDetails")}</DetailsSummary>
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
                  <Anchor key={label} href={url} target="_blank" rel="noreferrer">
                    {t(label)}
                  </Anchor>
                ),
            )}
          </div>
        </Details>
        {enabled.isError && <ErrorNotice error={enabled.error} />}
        {provider.descriptor.fixture && start.isError && (
          <ErrorNotice error={start.error} />
        )}
      </article>
    </Card>
  );
}
export function ProvidersPage({
  returnMediaId,
  returnQueue,
  returnToScraper = false,
  setup,
  embedded = false,
}: {
  returnMediaId?: string;
  returnQueue?: string[];
  returnToScraper?: boolean;
  setup?: ProviderSetupSearch;
  embedded?: boolean;
} = {}) {
  const { t } = useTranslation();
  const snapshot = useSnapshot(setup?.kind === "adult_video");
  return (
    <>
      {!embedded && (
        <PageHeading title={t("providers")}>
          {returnToScraper && (
            <Button asChild variant="outline">
              <Link
                to="/scraper"
                search={{ media: returnMediaId, queue: returnQueue }}
              >
                {t("scraperReturn")}
              </Link>
            </Button>
          )}
        </PageHeading>
      )}
      {snapshot.isPending ? (
        <Loading />
      ) : snapshot.isError ? (
        <ErrorNotice
          error={snapshot.error}
          retry={() => void snapshot.refetch()}
        />
      ) : snapshot.data.providers.length ? (
        <div className="grid items-start gap-5 2xl:grid-cols-2">
          {snapshot.data.providers.map((provider) => (
            <ProviderCard
              key={provider.descriptor.id}
              provider={provider}
              returnToScraper={returnToScraper}
              returnMediaId={returnMediaId}
              returnQueue={returnQueue}
              setup={setup}
            />
          ))}
        </div>
      ) : (
        <Empty title={t("noProviders")} />
      )}
    </>
  );
}
export function NetworkInspectorPage({
  embedded = false,
}: {
  embedded?: boolean;
} = {}) {
  const { t } = useTranslation();
  const { number } = useFormat();
  const snapshot = useSnapshot();
  return (
    <>
      {!embedded && <PageHeading title={t("networkInspector")} />}
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
        <Table>
          <TableHeader>
            <TableRow>
              {[
                "providerLabel",
                "providerRequest",
                "providerDuration",
                "providerStatus",
                "providerAttempt",
              ].map((key) => (
                <TableHead key={key}>{t(key)}</TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {snapshot.data.requests.map((record, index) => (
              <TableRow
                key={`${record.task_id}-${record.engine}-${record.attempt}-${record.method}-${record.safe_url}-${index}`}
              >
                <TableCell className="break-all text-xs">
                  {record.provider_id}
                  <p>{record.task_id}</p>
                </TableCell>
                <TableCell className="break-all text-xs">
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
                </TableCell>
                <TableCell>{number(record.duration_ms)} ms</TableCell>
                <TableCell>
                  {record.status === null
                    ? t("providerNoStatus")
                    : number(record.status)}
                  {record.failure && (
                    <p>{t(`providerFailure_${record.failure}`)}</p>
                  )}
                </TableCell>
                <TableCell>{number(record.attempt)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </>
  );
}
