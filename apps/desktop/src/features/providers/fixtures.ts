import type { ProviderSnapshot } from "../../api/providers";
export function fixtureSnapshot(): ProviderSnapshot {
  return {
    schema_version: 1,
    providers: [
      {
        descriptor: {
          id: "fixture.offline",
          name: "Offline fixture",
          description: "Explicitly offline fixture",
          website: null,
          terms_url: null,
          privacy_url: null,
          documentation_url: null,
          engine_types: ["api", "html"],
          requires_api_key: false,
          requires_login: false,
          rate_limit_policy: "No network",
          attribution: "Development fixture",
          data_usage_notes: "No media writes",
          license: "MIT",
          media_kinds: ["movie"],
          languages: ["zh-CN", "en-US"],
          capabilities: ["search"],
          fixture: true,
        },
        config: {
          enabled: true,
          revision: 1,
          timeout_ms: 10000,
          concurrency: 2,
          image_concurrency: 1,
          interval_ms: 250,
          max_response_bytes: 2097152,
          max_retries: 2,
          credential_ref: null,
        },
        health: "ready",
      },
    ],
    tasks: [],
    requests: [],
  };
}
export const fixtureTask = {
  id: "task-1",
  provider_id: "fixture.offline",
  state: "running" as const,
  query: "Fixture movie",
  created_at: "2026-11-03T00:00:00Z",
  updated_at: "2026-11-03T00:00:00Z",
  failure: null,
  result_summary: null,
};
