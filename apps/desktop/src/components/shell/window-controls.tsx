import { Copy, Minus, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/button";
import type { WindowChrome } from "./use-window-controls";
import "./window-translations";
import "./window-chrome.css";

export function WindowControls({ chrome }: { chrome: WindowChrome }) {
  const { t } = useTranslation();
  if (chrome.platform !== "windows") return null;
  const disabled = !chrome.ready || chrome.busy;
  const MaximizeIcon = chrome.maximized ? Copy : Square;
  return (
    <fieldset
      aria-label={t("windowControls")}
      className="window-controls"
      data-focused={chrome.focused}
      data-tauri-drag-region="false"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      <Button
        className="window-control"
        variant="ghost"
        size="icon"
        disabled={disabled}
        aria-label={t("windowMinimize")}
        onClick={() => void chrome.minimize()}
      >
        <Minus size={13} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      <Button
        className="window-control"
        variant="ghost"
        size="icon"
        disabled={disabled}
        aria-pressed={chrome.maximized}
        aria-label={t(chrome.maximized ? "windowRestore" : "windowMaximize")}
        onClick={() => void chrome.toggleMaximize()}
      >
        <MaximizeIcon size={12} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      <Button
        className="window-control window-close-control"
        variant="ghost"
        size="icon"
        disabled={disabled}
        aria-label={t("windowClose")}
        onClick={() => void chrome.close()}
      >
        <X size={14} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      {chrome.error && (
        <div className="window-controls-error" role="alert">
          <p>{t("windowOperationFailed")}</p>
          <Button
            className="mt-3"
            variant="outline"
            size="sm"
            disabled={chrome.busy}
            onClick={chrome.retry}
          >
            {t("windowRetry")}
          </Button>
        </div>
      )}
    </fieldset>
  );
}
