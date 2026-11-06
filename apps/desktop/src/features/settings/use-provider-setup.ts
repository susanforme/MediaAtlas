import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { libraryApi, type RootView } from "../../api/library";
import { providerApi } from "../../api/providers";
import { toast } from "../../components/ui/sonner";
import { usePrivacy } from "../library/context";
import { rootsQuery } from "../library/queries";
import { providersQuery } from "../providers/queries";
import type {
  ProviderSetupSearch,
  SetupComplete,
  SetupKind,
} from "./setup-search";

export function useSetupRoot(setup: ProviderSetupSearch) {
  const { includeAdult } = usePrivacy();
  const adult = setup.kind === "adult_video";
  const requested = setup.from === "library" && Boolean(setup.root);
  const roots = useQuery({
    ...rootsQuery(adult),
    enabled: requested && (!adult || includeAdult),
  });
  const root = roots.data?.find((entry) => entry.id === setup.root);
  return { roots, root, requested, allowed: !adult || includeAdult };
}

export function useProviderSetup(
  providerId: string,
  setup: ProviderSetupSearch,
  root: RootView | undefined,
  onComplete?: SetupComplete,
) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const { includeAdult } = usePrivacy();
  const adult = providerId === "dmm" || providerId === "wikidata";
  const active = useRef(false);
  const locked = useRef(false);
  const [phase, setPhase] = useState<"save" | "enable" | "start" | null>(null);
  const [error, setError] = useState<unknown>();
  const [failurePhase, setFailurePhase] = useState<"enable" | "start" | null>(
    null,
  );
  const getSnapshot = () =>
    adult ? adultApi.providers() : providerApi.snapshot();
  const snapshot = useQuery({
    ...providersQuery(adult),
    enabled: !adult || includeAdult,
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const provider = snapshot.data?.providers.find(
    (entry) => entry.descriptor.id === providerId && !entry.descriptor.fixture,
  );
  const kind = root?.kind ?? setup.kind;
  const incompatible = Boolean(
    kind && provider && !provider.descriptor.media_kinds.includes(kind),
  );
  const unavailable = !provider || incompatible || (adult && !includeAdult);
  async function run(
    credentialReady: boolean,
    saveCredential?: () => Promise<boolean>,
  ) {
    if (
      locked.current ||
      !provider ||
      unavailable ||
      (setup.root && !root?.enabled)
    )
      return;
    if (!credentialReady && !saveCredential) return;
    locked.current = true;
    setError(undefined);
    setFailurePhase(null);
    let nextPhase: "enable" | "start" = "enable";
    try {
      if (saveCredential) {
        setPhase("save");
        if (!(await saveCredential()) || !active.current) return;
      }
      if (!active.current) return;
      setPhase("enable");
      if (!provider.config.enabled)
        await providerApi.setEnabled(providerId, true);
      if (!active.current) return;
      const confirmed = await getSnapshot();
      if (!active.current) return;
      client.setQueryData(
        adult ? ["adult", "providers"] : ["providers"],
        confirmed,
      );
      const source = confirmed.providers.find(
        (entry) =>
          entry.descriptor.id === providerId && !entry.descriptor.fixture,
      );
      if (
        !source?.config.enabled ||
        (kind && !source.descriptor.media_kinds.includes(kind))
      )
        throw new DesktopError("invalid_response");
      if (setup.from === "library" && root) {
        nextPhase = "start";
        setPhase("start");
        const task = await libraryApi.sync(root.id);
        if (!active.current) return;
        if (
          task.root_id !== root.id ||
          task.mode !== "automatic" ||
          !["queued", "running", "succeeded", "waiting_for_user"].includes(
            task.state,
          )
        )
          throw new DesktopError("invalid_response");
        void client.invalidateQueries({
          queryKey: adult ? ["adult", "scan-tasks"] : ["tasks", false],
        });
        void client.invalidateQueries({ queryKey: ["summary", false] });
        toast.success(t("setupStarted"));
        onComplete?.(
          root.id,
          task.id,
          (root.kind ?? setup.kind) as SetupKind | undefined,
        );
      } else {
        toast.success(t("setupEnabled"));
        if (setup.from) onComplete?.(undefined, undefined, setup.kind);
      }
    } catch (failure) {
      if (active.current) {
        setError(failure);
        setFailurePhase(nextPhase);
      }
    } finally {
      locked.current = false;
      if (active.current) setPhase(null);
    }
  }
  return {
    snapshot,
    provider,
    incompatible,
    unavailable,
    phase,
    error,
    failurePhase,
    run,
  };
}
