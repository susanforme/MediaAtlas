import { useQueries, useQuery } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import type { MediaKind } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import {
  automaticRootSources,
  type RootProviderId,
  rootSourceState,
} from "./root-source-model";
import "./translations";

const sourceNames: Record<RootProviderId, string> = {
  tmdb: "TMDB",
  anilist: "AniList",
  dmm: "DMM",
  wikidata: "Wikidata",
};

export function RootSourceReadiness({
  kind,
  adultAvailable,
  disabled,
  onConfigure,
}: {
  kind: MediaKind | null;
  adultAvailable: boolean;
  disabled: boolean;
  onConfigure?: (provider: RootProviderId) => void;
}) {
  const { t } = useTranslation();
  const sources = automaticRootSources(kind);
  const adult = kind === "adult_video";
  const permitted = !adult || adultAvailable;
  const snapshot = useQuery({
    queryKey: adult ? ["adult", "providers"] : ["providers"],
    queryFn: adult ? adultApi.providers : providerApi.snapshot,
    enabled: sources.length > 0 && permitted,
  });
  const auth = useQueries({
    queries: sources.map((provider) => ({
      queryKey: adult
        ? ["adult", "provider-auth", provider]
        : ["provider-auth", provider],
      queryFn: () => providerAuthApi.status(provider),
      enabled:
        permitted &&
        snapshot.isSuccess &&
        snapshot.data.providers.some(
          (entry) =>
            entry.descriptor.id === provider &&
            !entry.descriptor.fixture &&
            (entry.descriptor.requires_api_key ||
              entry.descriptor.requires_login),
        ),
    })),
  });
  if (sources.length === 0)
    return (
      <p className="text-sm text-muted-foreground" role="status">
        {t("rootLocalTagsOnly")}
      </p>
    );
  const rows = sources.map((id, index) => {
    const provider = snapshot.data?.providers.find(
      (entry) => entry.descriptor.id === id,
    );
    const credential = auth[index];
    const needsCredential = Boolean(
      provider &&
        !provider.descriptor.fixture &&
        (provider.descriptor.requires_api_key ||
          provider.descriptor.requires_login),
    );
    return {
      id,
      name: provider?.descriptor.name ?? sourceNames[id],
      state: rootSourceState(
        provider,
        credential?.data,
        !permitted ||
          snapshot.isPending ||
          Boolean(needsCredential && credential?.isPending),
        snapshot.isError || Boolean(needsCredential && credential?.isError),
      ),
    };
  });
  const ready = adult
    ? rows.some((row) => row.state === "configured")
    : rows.every((row) => row.state === "configured");
  const checking = rows.some((row) => row.state === "checking");
  return (
    <Card asChild className="bg-muted">
      <section className="root-source-check" aria-label={t("rootSourceCheck")}>
        <div className="flex items-start gap-3">
          {ready ? (
            <CheckCircle2
              className="mt-0.5 shrink-0 text-primary"
              size={18}
              aria-hidden="true"
            />
          ) : (
            <CircleAlert
              className="mt-0.5 shrink-0 text-muted-foreground"
              size={18}
              aria-hidden="true"
            />
          )}
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium" role="status">
              {t(
                checking
                  ? "rootSourcesChecking"
                  : ready
                    ? "rootSourcesConfigured"
                    : "rootSourcesLocalFirst",
              )}
            </p>
            {!ready && !checking && (
              <p className="mt-1 text-sm text-muted-foreground">
                {t("rootSourcesMissingHelp")}
              </p>
            )}
            {adult && (
              <p className="mt-1 text-sm text-muted-foreground">
                {t("rootSourcesCatalogHelp")}
              </p>
            )}
            <ul className="mt-3 space-y-2">
              {rows.map((row) => (
                <li
                  className="flex flex-wrap items-center justify-between gap-2 text-sm"
                  key={row.id}
                >
                  <span>
                    {row.name}
                    <span className="ml-2 text-muted-foreground">
                      {t(`rootSource_${row.state}`)}
                    </span>
                  </span>
                  {row.state === "error" ? (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={disabled}
                      onClick={() => {
                        void snapshot.refetch();
                        for (const item of auth)
                          if (item.isError) void item.refetch();
                      }}
                    >
                      {t("retry")}
                    </Button>
                  ) : (
                    row.state !== "configured" &&
                    row.state !== "checking" &&
                    onConfigure && (
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={disabled || !permitted}
                        onClick={() => onConfigure(row.id)}
                      >
                        {t("rootConfigureSource", { source: row.name })}
                      </Button>
                    )
                  )}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </section>
    </Card>
  );
}
