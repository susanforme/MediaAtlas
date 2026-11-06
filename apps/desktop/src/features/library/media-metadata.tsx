import { LockKeyhole } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import { castNames, hasMetadataValue, metadataRatings, metadataRecord, metadataText } from "./media-metadata-model";
import { useFormat } from "./shared";

function MetadataValue({ field, value }: { field: string; value: unknown }) {
  const { t } = useTranslation();
  const { number } = useFormat();
  if (field === "cast" || field === "people") return <>{castNames(value).join(" / ")}</>;
  if (field === "ratings") return (
    <ul className="grid list-none gap-2 p-0">
      {metadataRatings(value).map((rating) => (
        <li key={`${rating.name}:${rating.value}:${rating.max}`} className="flex flex-wrap items-center gap-2">
          <span>{rating.name}</span>
          <Badge variant="secondary">{number(rating.value)} / {number(rating.max)}</Badge>
          {typeof rating.votes === "number" && Number.isSafeInteger(rating.votes) && rating.votes >= 0 && (
            <span className="text-muted-foreground">{t("videoDetailVotes", { count: rating.votes })}</span>
          )}
        </li>
      ))}
    </ul>
  );
  if (field === "collection" && metadataRecord(value)) return (
    <div><span>{metadataText(value.name)}</span>{metadataText(value.overview) && <p className="mt-2 text-muted-foreground">{metadataText(value.overview)}</p>}</div>
  );
  return <>{metadataText(value)}</>;
}

export function MediaCast({ names }: { names: string[] }) {
  return (
    <ul className="m-0 grid list-none grid-cols-[repeat(auto-fit,minmax(140px,1fr))] gap-3 p-0">
      {names.map((name) => <Card asChild key={name} className="p-4 sm:p-4"><li>{name}</li></Card>)}
    </ul>
  );
}

export function MediaMetadataFields({ fields }: { fields: {
  key: string; field: string; label: string; value: unknown;
  locale?: string | null; locked?: boolean; edited?: boolean; provider?: string | null;
}[] }) {
  const { t } = useTranslation();
  return (
    <dl className="[&>div]:border-b [&>div]:border-border [&>div]:py-4 [&_dt]:flex [&_dt]:flex-wrap [&_dt]:items-center [&_dt]:gap-2 [&_dt]:text-[14px] [&_dt]:font-medium [&_dd]:mt-2 [&_dd]:[overflow-wrap:anywhere]">
      {fields.filter((field) => hasMetadataValue(field.field, field.value)).map((field) => (
        <div key={field.key}>
          <dt>{field.label}
            {field.locale && <Badge variant="secondary">{field.locale}</Badge>}
            {field.locked && <Badge variant="secondary"><LockKeyhole size={12} aria-hidden="true" />{t("locked")}</Badge>}
            {field.edited && <Badge variant="secondary">{t("userEdited")}</Badge>}
          </dt>
          <dd className="whitespace-pre-wrap text-[14px]"><MetadataValue field={field.field} value={field.value} /></dd>
          {field.provider !== undefined && <dd className="text-[12px] text-muted-foreground">{t("source")}: {field.provider ?? t("localMetadata")}</dd>}
        </div>
      ))}
    </dl>
  );
}
