import { describe, expect, it } from "vitest";
import { DesktopError } from "../../api/desktop";
import type { ProviderFailure } from "../../api/providers";
import type { SearchPreview } from "../../api/scraper";
import { searchRecoveries } from "./recovery-model";

const preview = (failure?: ProviderFailure): SearchPreview => ({
  schema_version: 1,
  run_id: "run-1",
  candidates: [],
  failures: failure ? [{ provider_id: "tmdb", failure }] : [],
});

describe("search recovery decisions", () => {
  it.each([
    ["credentials_unavailable", "credentials"],
    ["disabled", "configuration"],
    ["invalid_configuration", "configuration"],
    ["rate_limited", "rate_limit"],
    ["cooling_down", "rate_limit"],
    ["timeout", "timeout"],
    ["dns_error", "network"],
    ["tls_error", "network"],
    ["network_error", "network"],
    ["http5xx", "network"],
    ["not_found", "no_results"],
    ["http4xx", "unknown"],
  ] as const)(
    "classifies %s without guessing HTTP details",
    (failure, kind) => {
      expect(searchRecoveries(preview(failure))).toEqual([
        { kind, providerId: "tmdb" },
      ]);
    },
  );

  it("only reports no results after a successful empty search", () => {
    expect(searchRecoveries()).toEqual([]);
    expect(searchRecoveries(preview())).toEqual([{ kind: "no_results" }]);
    expect(searchRecoveries(preview("timeout"))).not.toContainEqual({
      kind: "no_results",
    });
  });

  it("does not discard successful candidates because another provider failed", () => {
    const search: SearchPreview = {
      ...preview("timeout"),
      candidates: [
        {
          candidate: {
            kind: "movie",
            data: {
              provider_id: "anilist",
              remote_id: "1",
              title: "Title",
              original_title: null,
              year: 2026,
              aliases: [],
              external_ids: [],
              source_url: "https://example.test",
              language: "en-US",
            },
          },
          reasons: [],
          needs_review: true,
          duplicate_of: null,
        },
      ],
    };
    expect(searchRecoveries(search)).toEqual([
      { kind: "timeout", providerId: "tmdb" },
    ]);
    expect(search.candidates).toHaveLength(1);
    expect(searchRecoveries({ ...search, failures: [] })).toEqual([]);
  });

  it("reads only contract codes from wrapped or raw ErrorDto", () => {
    const envelope = {
      schema_version: 1 as const,
      code: "credentials_unavailable",
      message: "/private/Token secret",
      correlation_id: "secret-path",
    };
    expect(searchRecoveries(undefined, envelope)).toEqual([
      { kind: "credentials" },
    ]);
    expect(
      searchRecoveries(undefined, new DesktopError("request_failed", envelope)),
    ).toEqual([{ kind: "credentials" }]);
    expect(
      searchRecoveries(undefined, { ...envelope, schema_version: 2 }),
    ).toEqual([{ kind: "unknown" }]);
    expect(
      searchRecoveries(undefined, new Error("401 timeout /private/Token")),
    ).toEqual([{ kind: "unknown" }]);
    expect(
      searchRecoveries(undefined, { ...envelope, code: "__proto__" }),
    ).toEqual([{ kind: "unknown" }]);
  });

  it("retains desktop-required semantics and deduplicates repeated source failures", () => {
    expect(
      searchRecoveries(undefined, new DesktopError("desktop_required")),
    ).toEqual([{ kind: "desktop" }]);
    const search = preview("timeout");
    search.failures.push(...search.failures);
    expect(searchRecoveries(search)).toHaveLength(1);
  });
});
