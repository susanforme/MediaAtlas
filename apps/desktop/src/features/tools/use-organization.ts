import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import {
  type OrganizationPlan,
  type OrganizationStrategy,
  organizationApi,
} from "../../api/organization";

export function useOrganization(mediaId: string, adult: boolean) {
  const client = useQueryClient();
  const api = useMemo(() => organizationApi(adult), [adult]);
  const [target, setTarget] = useState("");
  const [template, setTemplate] = useState("{title} ({year})/{stem}.{ext}");
  const [strategy, setStrategy] = useState<OrganizationStrategy>("preserve");
  const [plan, setPlan] = useState<OrganizationPlan>();
  const [rollback, setRollback] = useState(false);
  const [historyId, setHistoryId] = useState("");
  const current = useRef<OrganizationPlan | undefined>(undefined);
  const active = useRef(true);
  const requestEpoch = useRef(0);
  const prefix = adult ? ["adult", "organization"] : ["organization"];
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
  });
  const history = useQuery({
    queryKey: [...prefix, "history"],
    queryFn: api.history,
    refetchInterval: 2500,
  });
  const journal = useQuery({
    queryKey: [...prefix, "logs", historyId],
    queryFn: () => api.logs(historyId),
    refetchInterval: 2500,
    enabled: !!historyId,
  });
  const savedPlan = useQuery({
    queryKey: [...prefix, "plan", historyId],
    queryFn: () => api.plan(historyId),
    enabled: !!historyId,
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      requestEpoch.current += 1;
      const old = current.current;
      if (old) void api.discard(old.id).catch(() => {});
    };
  }, [api]);
  function replace(
    value: OrganizationPlan,
    isRollback: boolean,
    epoch: number,
  ) {
    if (!active.current || epoch !== requestEpoch.current) {
      void api.discard(value.id).catch(() => {});
      return;
    }
    const old = current.current;
    if (old) void api.discard(old.id).catch(() => {});
    current.current = value;
    setPlan(value);
    setRollback(isRollback);
  }
  const preview = useMutation({
    mutationFn: async () => {
      discard();
      const epoch = requestEpoch.current;
      const value = await api.preview({
        media_item_id: mediaId,
        target_root_id: target,
        template,
        strategy,
      });
      return { value, epoch };
    },
    onSuccess: ({ value, epoch }) => replace(value, false, epoch),
  });
  const restore = useMutation({
    mutationFn: async (id: string) => {
      discard();
      const epoch = requestEpoch.current;
      const value = await api.rollbackPreview(id);
      return { value, epoch };
    },
    onSuccess: ({ value, epoch }) => replace(value, true, epoch),
  });
  const execute = useMutation({
    mutationFn: () =>
      rollback
        ? api.rollbackExecute(plan?.id ?? "")
        : api.execute(plan?.id ?? ""),
    onSuccess: () => {
      if (!active.current) return;
      requestEpoch.current += 1;
      current.current = undefined;
      setPlan(undefined);
      void client.invalidateQueries({ queryKey: prefix });
    },
  });
  const cancel = useMutation({
    mutationFn: () => api.cancel(plan?.id ?? ""),
    onSuccess: () => {
      if (!active.current) return;
      requestEpoch.current += 1;
      current.current = undefined;
      setPlan(undefined);
      void history.refetch();
    },
  });
  function discard() {
    requestEpoch.current += 1;
    const old = current.current;
    current.current = undefined;
    setPlan(undefined);
    if (old) void api.discard(old.id).catch(() => {});
  }
  return {
    rollback,
    target,
    setTarget,
    template,
    setTemplate,
    strategy,
    setStrategy,
    plan,
    historyId,
    setHistoryId,
    roots,
    history,
    journal,
    savedPlan,
    preview,
    restore,
    execute,
    cancel,
    discard,
  };
}
