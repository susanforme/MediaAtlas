import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { MediaFile } from "../../api/library";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";

/** Only loaded after a user asks to open media with multiple or missing files. */
export default function NativeOpenPicker({
  choices,
  pending,
  trigger,
  onSelect,
  onClose,
}: {
  choices: MediaFile[];
  pending: boolean;
  trigger: RefObject<HTMLButtonElement | null>;
  onSelect: (fileId: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        aria-describedby={undefined}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          trigger.current?.focus();
        }}
      >
        <DialogTitle className="mb-4 font-semibold">
          {t("nativeSelect")}
        </DialogTitle>
        {choices.length ? (
          <ul>
            {choices.map((file) => (
              <li key={file.id}>
                <Button
                  variant="outline"
                  disabled={pending}
                  onClick={() => onSelect(file.id)}
                >
                  {file.relative_path}
                  {file.part !== null ? ` · ${t("part")} ${file.part}` : ""}
                </Button>
              </li>
            ))}
          </ul>
        ) : (
          <p>{t("nativeNoFiles")}</p>
        )}
        <Button variant="outline" onClick={onClose}>
          {t("nativeClose")}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
