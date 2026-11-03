import { useMutation } from "@tanstack/react-query";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { libraryApi, type MediaFile } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import { Button } from "../../components/ui/button";
import { ErrorNotice } from "./shared";
import "./native-open-translations";
export function playableFiles(files: MediaFile[]) {
  return files.filter((f) =>
    /\.(mkv|mp4|avi|mov|wmv|m4v|webm|mpg|mpeg|ts|m2ts|vob|ogv)$/i.test(
      f.relative_path,
    ),
  );
}
/** A click is required for every dispatch; multiple versions/Parts are never guessed. */
export function NativeOpen({
  mediaId,
  files,
  adult = false,
  children,
  label,
  className,
}: {
  mediaId?: string;
  files?: MediaFile[];
  adult?: boolean;
  children?: React.ReactNode;
  label?: string;
  className?: string;
}) {
  const { t } = useTranslation();
  const [choices, setChoices] = useState<MediaFile[]>();
  const active = useRef(true);
  const context = useMemo(
    () => ({ mediaId, files, adult }),
    [mediaId, files, adult],
  );
  const currentContext = useRef(context);
  useLayoutEffect(() => {
    active.current = true;
    currentContext.current = context;
    setChoices(undefined);
    return () => {
      active.current = false;
    };
  }, [context]);
  const launch = useMutation({
    mutationFn: (fileId: string) => nativeOpenApi.open(fileId, adult),
  });
  const choose = useMutation({
    mutationFn: async () => {
      const started = currentContext.current;
      const list = playableFiles(
        files ??
          (adult
            ? await (await import("../../api/adult")).adultApi.getMedia(
                mediaId ?? "",
              )
            : await libraryApi.detail(mediaId ?? "", false)
          ).files,
      );
      if (!active.current || started !== currentContext.current) return;
      if (list.length === 1) await launch.mutateAsync(list[0].id);
      else setChoices(list);
    },
  });
  return (
    <>
      <Button
        className={className}
        onClick={() => choose.mutate()}
        disabled={choose.isPending || launch.isPending}
        aria-label={
          label || typeof children === "string"
            ? `${t("nativeOpen")}: ${label ?? String(children)}`
            : t("nativeOpen")
        }
      >
        {children ?? t("nativeOpen")}
      </Button>
      {choices && (
        <section
          role="dialog"
          aria-label={t("nativeSelect")}
          className="panel mt-3"
        >
          <h3>{t("nativeSelect")}</h3>
          {choices.length ? (
            <ul>
              {choices.map((f) => (
                <li key={f.id}>
                  <Button
                    variant="outline"
                    disabled={launch.isPending}
                    onClick={() => launch.mutate(f.id)}
                  >
                    {f.relative_path}
                    {f.part !== null ? ` · ${t("part")} ${f.part}` : ""}
                  </Button>
                </li>
              ))}
            </ul>
          ) : (
            <p>{t("nativeNoFiles")}</p>
          )}
          <Button variant="outline" onClick={() => setChoices(undefined)}>
            {t("nativeClose")}
          </Button>
        </section>
      )}
      {(choose.error || launch.error) && (
        <ErrorNotice error={choose.error ?? launch.error} />
      )}
      {launch.isSuccess && <p role="status">{t("nativeRequested")}</p>}
    </>
  );
}
