import { useQueryClient } from "@tanstack/react-query";
import { createContext, type ReactNode, useContext, useState } from "react";

export const ADULT_STORAGE_KEY = "mediaatlas.adultEnabled";
const PrivacyContext = createContext({
  includeAdult: false,
  setIncludeAdult: (_value: boolean) => {},
});
export function PrivacyProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const [includeAdult, setAdult] = useState(() => {
    try {
      return localStorage.getItem(ADULT_STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });
  function setIncludeAdult(value: boolean) {
    // Drop details, posters, tasks and candidates immediately when privacy changes.
    queryClient.clear();
    setAdult(value);
    try {
      localStorage.setItem(ADULT_STORAGE_KEY, String(value));
    } catch {
      /* In-memory privacy still applies. */
    }
  }
  return (
    <PrivacyContext.Provider value={{ includeAdult, setIncludeAdult }}>
      {children}
    </PrivacyContext.Provider>
  );
}
export function usePrivacy() {
  return useContext(PrivacyContext);
}
