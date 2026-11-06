import { Copy, Minus, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert } from "../ui/alert";
import { Button } from "../ui/button";
import type { WindowChrome } from "./use-window-controls";
import "./window-translations";

export function WindowControls({ chrome }: { chrome: WindowChrome }) {
  const { t } = useTranslation();
  if (chrome.platform !== "windows" && chrome.platform !== "linux") return null;
  const disabled = !chrome.ready || chrome.busy;
  const MaximizeIcon = chrome.maximized ? Copy : Square;
  return (
    <fieldset
      aria-label={t("windowControls")}
      className="window-controls absolute top-0 right-0 z-40 m-0 flex h-9 w-[138px] min-w-0 select-none border-0 p-0 text-foreground data-[focused=false]:text-muted-foreground"
      data-focused={chrome.focused}
      data-tauri-drag-region="false"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      <Button
        className="h-9 w-[46px] rounded-none border-0 bg-transparent p-0 text-inherit [&:hover:not(:disabled)]:bg-hover [&:hover:not(:disabled)]:text-foreground"
        variant="ghost"
        size="icon"
        disabled={disabled}
        aria-label={t("windowMinimize")}
        onClick={() => void chrome.minimize()}
      >
        <Minus size={13} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      <Button
        className="h-9 w-[46px] rounded-none border-0 bg-transparent p-0 text-inherit [&:hover:not(:disabled)]:bg-hover [&:hover:not(:disabled)]:text-foreground"
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
        className="h-9 w-[46px] rounded-none border-0 bg-transparent p-0 text-inherit [&:hover:not(:disabled)]:bg-destructive [&:hover:not(:disabled)]:text-destructive-foreground"
        variant="ghost"
        size="icon"
        disabled={disabled}
        aria-label={t("windowClose")}
        onClick={() => void chrome.close()}
      >
        <X size={14} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      {chrome.error && (
        <Alert
          variant="destructive"
          className="absolute top-[42px] right-2 w-[min(300px,calc(100vw_-_32px))] bg-popover text-[13px] leading-[var(--text-sm--line-height)] text-popover-foreground shadow-lg"
        >
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
        </Alert>
      )}
    </fieldset>
  );
}
