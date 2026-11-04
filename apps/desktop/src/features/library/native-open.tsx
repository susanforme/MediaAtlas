import { useMutation } from "@tanstack/react-query";
import {
  lazy,
  Suspense,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { libraryApi, type MediaFile } from "../../api/library";
import { nativeOpenApi } from "../../api/organization";
import { Button } from "../../components/ui/button";
import { toast } from "../../components/ui/sonner";
import { ErrorNotice } from "./shared";
import "./native-open-translations";

const NativeOpenPicker = lazy(() => import("./native-open-picker"));
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
  const trigger = useRef<HTMLButtonElement>(null);
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
    mutationFn: async (fileId: string) => {
      const started = currentContext.current;
      const result = await nativeOpenApi.open(fileId, adult);
      if (active.current && started === currentContext.current) {
        setChoices(undefined);
        if (result.requested) toast.success(t("nativeRequested"));
      }
      return result;
    },
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
        ref={trigger}
        className={className}
        onClick={() => choose.mutate()}
        disabled={choose.isPending || launch.isPending || choices !== undefined}
        aria-label={
          label || typeof children === "string"
            ? `${t("nativeOpen")}: ${label ?? String(children)}`
            : t("nativeOpen")
        }
      >
        {children ?? t("nativeOpen")}
      </Button>
      {choices && (
        <Suspense fallback={<p role="status">{t("loading")}</p>}>
          <NativeOpenPicker
            choices={choices}
            pending={launch.isPending}
            trigger={trigger}
            onSelect={(fileId) => launch.mutate(fileId)}
            onClose={() => setChoices(undefined)}
          />
        </Suspense>
      )}
      {(choose.error || launch.error) && (
        <ErrorNotice error={choose.error ?? launch.error} />
      )}
    </>
  );
}
