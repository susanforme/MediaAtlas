import { useTranslation } from "react-i18next";
import { Toaster as Sonner } from "sonner";
import { useTheme } from "../../lib/theme";
import "./sonner-translations";

export { toast } from "sonner";

export function Toaster() {
  const { t } = useTranslation();
  const theme = useTheme();
  return (
    <Sonner
      theme={theme}
      position="bottom-right"
      closeButton
      visibleToasts={3}
      containerAriaLabel={t("notifications")}
      toastOptions={{
        closeButtonAriaLabel: t("dismissNotification"),
        className: "bg-surface! text-foreground! border-border! [font-family:inherit]!",
      }}
    />
  );
}
