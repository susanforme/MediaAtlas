import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type MusicReleaseGraph,
  musicApi,
  type TagPreview,
  type TagResult,
} from "../../api/music";
import { toast } from "../../components/ui/sonner";
import { tagDefaults } from "./tag-model";

export function useTagEditor(fileId: string, graph: MusicReleaseGraph) {
  const { t } = useTranslation();
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const tags = useQuery({
    queryKey: ["music-tags", fileId],
    queryFn: () => musicApi.read(fileId),
  });
  const history = useQuery({
    queryKey: ["music-tag-history", fileId],
    queryFn: () => musicApi.history(fileId),
  });
  const [values, setValues] = useState(() => tagDefaults(graph, fileId));
  const [selected, setSelected] = useState<string[]>([]);
  const [preview, setPreview] = useState<TagPreview>();
  const [restore, setRestore] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [operation, setOperation] = useState<TagResult>();
  useEffect(() => {
    const id = preview?.id;
    if (!id) return;
    return () => {
      void musicApi.discard(id).catch(() => {});
    };
  }, [preview?.id]);
  const plan = useMutation({
    mutationFn: () =>
      musicApi.preview(
        fileId,
        Object.fromEntries(selected.map((f) => [f, values[f]])),
      ),
    onSuccess: (p) => {
      setPreview(p);
      setRestore(false);
      setConfirmed(false);
    },
  });
  const write = useMutation({
    mutationFn: () =>
      restore
        ? musicApi.restoreExecute(preview?.id ?? "")
        : musicApi.execute(preview?.id ?? ""),
    onSuccess: (r) => {
      if (!mounted.current) return;
      const message = t("musicWritten", {
        state: t(`musicState_${r.state}`, { defaultValue: t("musicUnknown") }),
      });
      if (r.state === "succeeded") toast.success(message);
      else toast.error(message);
      setOperation(r);
      setPreview(undefined);
      setConfirmed(false);
      void tags.refetch();
      void history.refetch();
    },
  });
  const restorePlan = useMutation({
    mutationFn: musicApi.restorePreview,
    onSuccess: (p) => {
      setPreview(p);
      setRestore(true);
      setConfirmed(false);
    },
  });
  const busy = plan.isPending || write.isPending || restorePlan.isPending;
  return {
    tags,
    history,
    values,
    setValues,
    selected,
    setSelected,
    preview,
    setPreview,
    restore,
    confirmed,
    setConfirmed,
    operation,
    plan,
    write,
    restorePlan,
    busy,
  };
}
