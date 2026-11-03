import { request } from "./library";
export interface AdultSettings {
  entry_visible: boolean;
  covers_visible: boolean;
  remember_recent: boolean;
  unlocked: boolean;
}
export function adultSettings(v: unknown): v is AdultSettings {
  return (
    v !== null &&
    typeof v === "object" &&
    !Array.isArray(v) &&
    ["entry_visible", "covers_visible", "remember_recent", "unlocked"].every(
      (key) => typeof (v as Record<string, unknown>)[key] === "boolean",
    )
  );
}
export const adultPrivacyApi = {
  settings: () => request("adult_settings", {}, adultSettings),
  saveSettings: (value: Omit<AdultSettings, "unlocked">) =>
    request("adult_save_settings", value, adultSettings),
  unlock: () => request("adult_unlock", {}, adultSettings),
  lock: () => request("adult_lock", {}, adultSettings),
};
