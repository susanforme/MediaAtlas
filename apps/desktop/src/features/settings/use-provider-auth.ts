import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DesktopError } from "../../api/desktop";
import {
  type AuthMethod,
  type AuthStorage,
  providerAuthApi,
} from "../../api/provider-auth";
import { toast } from "../../components/ui/sonner";
import { providerAuthQuery } from "../providers/queries";

export function useProviderAuth(providerId: string) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const active = useRef(false);
  const operation = useRef(false);
  const [method, setMethod] = useState<AuthMethod>("bearer");
  const [storage, setStorage] = useState<AuthStorage>("system");
  const [secret, setSecret] = useState("");
  const [affiliate, setAffiliate] = useState("");
  const [pending, setPending] = useState<"save" | "clear" | null>(null);
  const [outcome, setOutcome] = useState<"save_failed" | "clear_failed" | null>(
    null,
  );
  const [error, setError] = useState<unknown>();
  const [invalid, setInvalid] = useState(false);
  const status = useQuery({
    ...providerAuthQuery(providerId),
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
    await saveCredential();
  }
  async function saveCredential(notify = true): Promise<boolean> {
    if (operation.current) return false;
    const submitted =
      method === "affiliate"
        ? JSON.stringify({
            api_id: secret.trim(),
            affiliate_id: affiliate.trim(),
          })
        : secret.trim();
    const valid =
      secret.trim().length > 0 &&
      (method !== "affiliate" || affiliate.trim().length > 0) &&
      !/\s/.test(submitted) &&
      [...submitted].every(
        (character) =>
          character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127,
      );
    setInvalid(!valid);
    if (!valid) {
      setSecret("");
      setAffiliate("");
      return false;
    }
    setSecret("");
    setAffiliate("");
    operation.current = true;
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
        if (notify && saved.status !== "cleanup_pending")
          toast.success(t("providerAuthSaved"));
        return true;
      }
      return false;
    } catch (failure: unknown) {
      if (active.current) {
        setError(failure);
        setOutcome("save_failed");
      }
      return false;
    } finally {
      operation.current = false;
      if (active.current) {
        setSecret("");
        setAffiliate("");
        setPending(null);
      }
    }
  }
  async function clear() {
    if (operation.current) return;
    operation.current = true;
    setSecret("");
    setAffiliate("");
    setPending("clear");
    setOutcome(null);
    setError(undefined);
    setInvalid(false);
    try {
      const cleared = await providerAuthApi.clear(providerId);
      if (cleared.configured) throw new DesktopError("invalid_response");
      if (active.current) {
        client.setQueryData(["provider-auth", providerId], cleared);
        if (cleared.status !== "cleanup_pending")
          toast.success(t("providerAuthCleared"));
      }
    } catch (failure: unknown) {
      if (active.current) {
        setError(failure);
        setOutcome("clear_failed");
      }
    } finally {
      operation.current = false;
      if (active.current) {
        setSecret("");
        setAffiliate("");
        setPending(null);
      }
    }
  }
  return {
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
  };
}
