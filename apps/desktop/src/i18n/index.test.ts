import { beforeEach, expect, it } from "vitest";
import i18n, {
  LANGUAGE_STORAGE_KEY,
  persistLanguage,
  readStoredLanguage,
  resources,
} from "./index";

beforeEach(() => localStorage.clear());
it("uses zh-CN by default and persists only the supported locale IDs", async () => {
  expect(readStoredLanguage()).toBe("zh-CN");
  await i18n.changeLanguage("en-US");
  expect(readStoredLanguage()).toBe("en-US");
  expect(localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe("en-US");
  localStorage.setItem(LANGUAGE_STORAGE_KEY, "en");
  expect(readStoredLanguage()).toBe("en-US");
  persistLanguage("unsupported");
  expect(readStoredLanguage()).toBe("zh-CN");
  expect(Object.keys(resources)).toEqual(["zh-CN", "en-US"]);
});
it("falls back to US English when a Chinese translation is absent", async () => {
  i18n.addResource(
    "en-US",
    "translation",
    "fixtureEnglishOnly",
    "Fixture English fallback",
  );
  await i18n.changeLanguage("zh-CN");
  expect(i18n.t("fixtureEnglishOnly")).toBe("Fixture English fallback");
  expect(
    i18n.getResource("zh-CN", "translation", "fixtureEnglishOnly"),
  ).toBeUndefined();
});
