import { useTranslation } from "react-i18next";
import { Toaster as Sonner } from "sonner";
import "./sonner.css";
import "./sonner-translations";

export { toast } from "sonner";

export function Toaster() {
  const { t } = useTranslation();
  return (
    <Sonner
      theme="dark"
      position="bottom-right"
      closeButton
      visibleToasts={3}
      containerAriaLabel={t("notifications")}
      toastOptions={{
        closeButtonAriaLabel: t("dismissNotification"),
        style: {
          background: "var(--color-surface)",
          color: "var(--color-foreground)",
          borderColor: "var(--color-border)",
          fontFamily: "inherit",
        },
      }}
    />
  );
}
