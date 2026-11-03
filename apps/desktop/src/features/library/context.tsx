import { useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
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
});
export function PrivacyProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const [settings, setSettings] = useState(defaults);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>();
  const generation = useRef(0);
  useEffect(() => {
    let active = true;
    const epoch = generation.current;
    void adultApi
      .settings()
      .then((value) => {
        if (active && epoch === generation.current) setSettings(value);
      })
      .catch(() => {
        /* Browser previews keep privacy closed. */
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
      generation.current += 1;
    };
  }, []);
  function clearPrivate() {
    void queryClient.cancelQueries();
    queryClient.clear();
  }
  async function saveSettings(value: Omit<AdultSettings, "unlocked">) {
    const epoch = ++generation.current;
    setError(undefined);
    if (!value.entry_visible) {
      clearPrivate();
      setSettings((previous) => ({ ...previous, ...value, unlocked: false }));
    }
    try {
      const result = await adultApi.saveSettings({
        entry_visible: value.entry_visible,
        covers_visible: value.covers_visible,
        remember_recent: value.remember_recent,
      });
      if (epoch === generation.current) setSettings(result);
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
    }
  }
  function setIncludeAdult(value: boolean) {
    void saveSettings({ ...settings, entry_visible: value });
  }
  async function unlock() {
    const epoch = ++generation.current;
    setError(undefined);
    try {
      const result = await adultApi.unlock();
      if (epoch === generation.current) setSettings(result);
    } catch (failure) {
      if (epoch === generation.current) setError(failure);
    }
  }
  async function lock() {
    const epoch = ++generation.current;
    clearPrivate();
    setSettings((previous) => ({ ...previous, unlocked: false }));
    setError(undefined);
    try {
      const result = await adultApi.lock();
      if (epoch === generation.current)
        setSettings({ ...result, unlocked: false });
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
      }}
    >
      {children}
    </PrivacyContext.Provider>
  );
}
export function usePrivacy() {
  return useContext(PrivacyContext);
}
