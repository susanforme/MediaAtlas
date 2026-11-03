import { ImageOff } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

export function MediaArtwork({
  uri,
  title,
  eager = false,
}: {
  uri: string | null | undefined;
  title: string;
  eager?: boolean;
}) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState<string>();
  // Cached poster IPC is data-only. Never load arbitrary local paths from metadata.
  const safe =
    typeof uri === "string" && uri.startsWith("data:image/") && failed !== uri;
  return safe ? (
    <img
      src={uri}
      alt={title}
      loading={eager ? "eager" : "lazy"}
      onError={() => setFailed(uri)}
    />
  ) : (
    <div className="poster-placeholder">
      <ImageOff size={28} aria-hidden="true" />
      <span>{t("noPoster")}</span>
    </div>
  );
}
