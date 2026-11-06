import { useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  type AdultSettings,
  adultPrivacyApi as adultApi,
} from "../../api/adult-privacy";

// Legacy preference is deliberately not read: an old visible entry never unlocks a session.
export const ADULT_STORAGE_KEY = "mediaatlas.adultEnabled";
const defaults: AdultSettings = {
  entry_visible: false,
  covers_visible: false,
  remember_recent: false,
  unlocked: false,
};
const PrivacyContext = createContext({
  includeAdult: false,
  settings: defaults,
  loading: false,
  error: undefined as unknown,
  setIncludeAdult: (_value: boolean) => {},
  saveSettings: async (_value: Omit<AdultSettings, "unlocked">) => {},
  unlock: async () => {},
  lock: async () => {},
  readSettings: () => defaults,
});
export function PrivacyProvider({ children, onAccessRevoked }: { children: ReactNode; onAccessRevoked?: () => void }) {
  const queryClient = useQueryClient();
  const [settings, setSettings] = useState(defaults);
  const currentSettings = useRef(defaults);
  const publishSettings = useCallback((update: AdultSettings | ((previous: AdultSettings) => AdultSettings)) => {
    const next = typeof update === "function" ? update(currentSettings.current) : update;
    currentSettings.current = next;
    setSettings(next);
  }, []);
  const readSettings = useCallback(() => currentSettings.current, []);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>();
  const generation = useRef(0);
  const settingsWrites = useRef<Promise<unknown>>(Promise.resolve());
  const persistSettings = useCallback(
    (value: Omit<AdultSettings, "unlocked">) => {
      // Keep backend preference writes ordered too: ignoring a stale response
      // alone would not stop a late enable from persisting after a disable.
      const pending = settingsWrites.current
        .catch(() => {})
        .then(() => adultApi.saveSettings(value));
      settingsWrites.current = pending;
      return pending;
    },
    [],
  );
  useEffect(() => {
    let active = true;
    const epoch = generation.current;
    void adultApi
      .settings()
      .then(async (value) => {
        if (!active || epoch !== generation.current) return;
        // The persisted content-type setting is the user's opt-in. Establish
        // the revocable backend session without a second frontend unlock flow.
        publishSettings(value);
        const preferences =
          value.entry_visible &&
          (!value.covers_visible || value.remember_recent)
            ? await persistSettings({
                entry_visible: true,
                covers_visible: true,
                remember_recent: false,
              })
            : value;
        if (!active || epoch !== generation.current) return;
        const current =
          preferences.entry_visible && !preferences.unlocked
            ? await adultApi.unlock()
            : preferences;
        if (active && epoch === generation.current) publishSettings(current);
      })
      .catch((failure) => {
        if (active && epoch === generation.current) setError(failure);
      })
      .finally(() => {
        if (active && epoch === generation.current) setLoading(false);
      });
    return () => {
      active = false;
      generation.current += 1;
    };
  }, [persistSettings, publishSettings]);
  function clearPrivate() {
    currentSettings.current = { ...currentSettings.current, unlocked: false };
    void queryClient.cancelQueries();
    queryClient.clear();
    onAccessRevoked?.();
  }
  async function saveSettings(value: Omit<AdultSettings, "unlocked">) {
    const epoch = ++generation.current;
    setError(undefined);
    setLoading(true);
    if (!value.entry_visible) {
      clearPrivate();
      publishSettings((previous) => ({ ...previous, ...value, unlocked: false }));
    }
    try {
      const result = await persistSettings({
        entry_visible: value.entry_visible,
        covers_visible: value.covers_visible,
        remember_recent: value.remember_recent,
      });
      if (epoch !== generation.current) return;
      publishSettings(result);
      if (result.entry_visible && !result.unlocked) {
        const current = await adultApi.unlock();
        if (epoch === generation.current) publishSettings(current);
      }
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
    } finally {
      if (epoch === generation.current) setLoading(false);
    }
  }
  function setIncludeAdult(value: boolean) {
    void saveSettings({
      entry_visible: value,
      covers_visible: value,
      remember_recent: false,
    });
  }
  async function unlock() {
    const epoch = ++generation.current;
    setError(undefined);
    try {
      const result = await adultApi.unlock();
      if (epoch === generation.current) publishSettings(result);
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
    }
  }
  async function lock() {
    const epoch = ++generation.current;
    clearPrivate();
    publishSettings((previous) => ({ ...previous, unlocked: false }));
    setError(undefined);
    try {
      const result = await adultApi.lock();
      if (epoch === generation.current)
        publishSettings({ ...result, unlocked: false });
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
    }
  }
  return (
    <PrivacyContext.Provider
      value={{
        includeAdult: settings.entry_visible && settings.unlocked,
        settings,
        loading,
        error,
        setIncludeAdult,
        saveSettings,
        unlock,
        lock,
        readSettings,
      }}
    >
      {children}
    </PrivacyContext.Provider>
  );
}
export function usePrivacy() {
  return useContext(PrivacyContext);
}
