import type { RoutePreparation } from "../../lib/route-transition";
import { rootsQuery } from "../library/queries";
import { maintenanceStatusQuery } from "../maintenance/queries";
import { providerAuthQuery, providersQuery } from "../providers/queries";
import { preloadSettingsTab } from "./page";
import { PUBLIC_SETUP_PROVIDERS, type ProviderSetupSearch, suggestedProvider } from "./setup-search";

export async function prepareSettingsPage(context: RoutePreparation, setup: ProviderSetupSearch) {
  context.check();
  await Promise.all([preloadSettingsTab(setup), prepareSettingsData(context, setup)]);
  context.check();
}

async function prepareSettingsData(context: RoutePreparation, setup: ProviderSetupSearch) {
  const tab = setup.tab ?? (setup.from || setup.provider ? "auth" : "general");
  const settings = context.readSettings();
  const adultAllowed = Boolean(settings?.entry_visible && settings.unlocked);
  const adult = setup.kind === "adult_video";
  if (tab === "providers" || tab === "network") {
    if (tab === "network" || !adult || adultAllowed)
      await context.queryClient.ensureQueryData(providersQuery(tab === "providers" && adult));
  } else if (tab === "diagnostics") {
    await context.queryClient.ensureQueryData(maintenanceStatusQuery);
  } else if (tab === "auth") {
    if (setup.from === "library" && setup.root && adult && !adultAllowed) return;
    const roots = setup.from === "library" && setup.root
      ? await context.queryClient.ensureQueryData(rootsQuery(adult)) : [];
    context.check();
    const root = roots.find((entry) => entry.id === setup.root);
    const requested = setup.provider ?? suggestedProvider(root?.kind ?? setup.kind);
    const provider = !adultAllowed && ["dmm", "wikidata"].includes(requested) ? "tmdb" : requested;
    await Promise.all([
      context.queryClient.ensureQueryData(providersQuery(["dmm", "wikidata"].includes(provider))),
      ...(!PUBLIC_SETUP_PROVIDERS.some((id) => id === provider) ? [context.queryClient.ensureQueryData(providerAuthQuery(provider))] : []),
    ]);
  }
  context.check();
}
