import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import { initializeTheme } from "./lib/theme";
import "./i18n";
import "./styles.css";

initializeTheme();
const root = document.getElementById("root");
if (!root) throw new Error("Missing application root");
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
