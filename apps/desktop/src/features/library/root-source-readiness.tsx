import { useQueries, useQuery } from "@tanstack/react-query";
import { ArrowDown, ArrowUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import type { MediaKind } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { ErrorNotice, Loading } from "./shared";
import {
  automaticRootSources,
  directorySourceChoices,
  moveRootSource,
  type RootProviderId,
  rootSourceState,
} from "./root-source-model";
import "./translations";

export function RootSourceReadiness({
  kind, adultAvailable, disabled, value, onValueChange, onConfigure,
}: {
  kind: MediaKind | null;
  adultAvailable: boolean;
  disabled: boolean;
  value: string[] | null;
  onValueChange: (sources: string[]) => void;
  onConfigure?: (provider: RootProviderId) => void;
}) {
  const { t } = useTranslation();
  const adult = kind === "adult_video";
  const permitted = !adult || adultAvailable;
  const snapshot = useQuery({
    queryKey: adult ? ["adult", "providers"] : ["providers"],
    queryFn: adult ? adultApi.providers : providerApi.snapshot,
    enabled: kind !== "music_release" && permitted,
  });
  const providers = directorySourceChoices(snapshot.data?.providers ?? [], kind);
  const selected = value ?? automaticRootSources(kind);
  const ordered = [
    ...selected,
    ...providers.map((entry) => entry.descriptor.id).filter((id) => !selected.includes(id)),
  ];
  const auth = useQueries({
    queries: providers.map((entry) => ({
      queryKey: adult
        ? ["adult", "provider-auth", entry.descriptor.id]
        : ["provider-auth", entry.descriptor.id],
      queryFn: () => providerAuthApi.status(entry.descriptor.id),
      enabled: permitted && (entry.descriptor.requires_api_key || entry.descriptor.requires_login),
    })),
  });
  if (kind === "music_release")
    return <p className="text-sm text-muted-foreground">{t("rootLocalTagsOnly")}</p>;
  return (
    <Card asChild className="bg-muted p-4 sm:p-4">
      <section aria-label={t("rootSourceCheck")}>
        <h3 className="text-sm font-medium">{t("rootSourceCheck")}</h3>
        <p className="mt-2 text-sm text-muted-foreground">{t("rootSourcePriorityHelp")}</p>
        {!permitted ? (
          <p role="status" className="mt-3 text-sm">{t("privacyHidden")}</p>
        ) : snapshot.isPending ? <Loading /> : snapshot.isError ? (
          <ErrorNotice error={snapshot.error} retry={() => void snapshot.refetch()} />
        ) : (
          <ol className="mt-4 space-y-2">
            {ordered.map((id) => {
              const index = selected.indexOf(id);
              const sourceIndex = providers.findIndex((entry) => entry.descriptor.id === id);
              const provider = providers[sourceIndex];
              const credential = auth[sourceIndex];
              const name = provider?.descriptor.name ?? id;
              const checked = index >= 0;
              const state = rootSourceState(
                provider, credential?.data,
                Boolean(credential?.isLoading), Boolean(credential?.isError),
              );
              return (
                <li key={id}>
                  <Card className="flex flex-wrap items-center gap-2 rounded-lg p-3 sm:p-3">
                    <Label className="flex min-w-0 flex-1 items-center gap-3">
                      <Checkbox
                        checked={checked}
                        disabled={disabled || !permitted}
                        onChange={(event) => onValueChange(
                          event.target.checked ? [...selected, id] : selected.filter((entry) => entry !== id),
                        )}
                      />
                      {checked && <span className="text-xs text-muted-foreground" aria-hidden="true">{index + 1}</span>}
                      <span>{name}</span>
                    </Label>
                    {checked && (
                      <>
                        <Button
                          variant="ghost" size="icon"
                          aria-label={t("rootSourceMoveUp", { source: name })}
                          disabled={disabled || index === 0}
                          onClick={() => onValueChange(moveRootSource(selected, index, -1))}
                        >
                          <ArrowUp size={16} aria-hidden="true" />
                        </Button>
                        <Button
                          variant="ghost" size="icon"
                          aria-label={t("rootSourceMoveDown", { source: name })}
                          disabled={disabled || index === selected.length - 1}
                          onClick={() => onValueChange(moveRootSource(selected, index, 1))}
                        >
                          <ArrowDown size={16} aria-hidden="true" />
                        </Button>
                      </>
                    )}
                    {checked && state !== "configured" && (
                      <span className="text-xs text-muted-foreground">{t("rootSource_" + state)}</span>
                    )}
                    {checked && (state === "disabled" || state === "credentials") && onConfigure && (
                      <Button size="sm" variant="outline" disabled={disabled} onClick={() => onConfigure(id as RootProviderId)}>
                        {t("rootConfigureSource", { source: name })}
                      </Button>
                    )}
                    {checked && state === "error" && credential && (
                      <Button size="sm" variant="outline" disabled={disabled} onClick={() => void credential.refetch()}>{t("retry")}</Button>
                    )}
                  </Card>
                </li>
              );
            })}
          </ol>
        )}
      </section>
    </Card>
  );
}
