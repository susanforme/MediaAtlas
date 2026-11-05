import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isNfoStartup } from "./api/nfo";
import { App } from "./app";
import { ErrorNotice } from "./features/library/shared";
import { NfoStandalone } from "./features/nfo/page";
import i18n from "./i18n";
import { initializeTheme } from "./lib/theme";

initializeTheme();
document.documentElement.lang = i18n.resolvedLanguage ?? "zh-CN";
document.addEventListener("contextmenu", (event) => event.preventDefault());
const root = document.getElementById("root");
if (!root) throw new Error("Missing application root");
const appRoot = createRoot(root);
async function start() {
  const nfo = await isNfoStartup();
  appRoot.render(
    <StrictMode>{nfo ? <NfoStandalone /> : <App />}</StrictMode>,
  );
}
void start().catch((error: unknown) => {
  appRoot.render(
    <main className="p-6">
      <ErrorNotice error={error} retry={() => window.location.reload()} />
    </main>,
  );
});
