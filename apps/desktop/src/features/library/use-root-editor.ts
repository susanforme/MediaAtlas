import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi, type RootInput, type RootView } from "../../api/library";
import { usePrivacy } from "./context";

const newRoot = (): RootInput => ({
  id: null,
  name: "",
  path: "",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
});
export function useRootEditor(adult: boolean) {
  const { includeAdult: unlockedAdult } = usePrivacy();
  const client = useQueryClient();
  const [form, setForm] = useState<RootInput | null>(null);
  const [excludes, setExcludes] = useState("");
  const [remove, setRemove] = useState<RootView | null>(null);
  const [validation, setValidation] = useState(false);
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
    enabled: !adult || unlockedAdult,
  });
  const save = useMutation({
    mutationFn: (input: RootInput) => libraryApi.saveRoot(input),
    onSuccess: () => {
      setForm(null);
      void client.invalidateQueries();
    },
  });
  const deletion = useMutation({
    mutationFn: (id: string) => libraryApi.removeRoot(id),
    onSuccess: () => {
      setRemove(null);
      void client.invalidateQueries();
    },
  });
  function edit(root?: RootView) {
    setValidation(false);
    save.reset();
    const value = root
      ? {
          id: root.id,
          name: root.name,
          path: root.path,
          kind: root.kind,
          enabled: root.enabled,
          exclude_patterns: root.exclude_patterns,
        }
      : {
          ...newRoot(),
          kind: adult ? ("adult_video" as const) : ("movie" as const),
        };
    setForm(value);
    setExcludes(value.exclude_patterns.join("\n"));
  }
  function submit(event: FormEvent) {
    event.preventDefault();
    if (!form?.name.trim() || !form.path.trim()) {
      setValidation(true);
      return;
    }
    save.mutate({
      ...form,
      exclude_patterns: excludes
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean),
    });
  }
  return {
    form,
    setForm,
    excludes,
    setExcludes,
    remove,
    setRemove,
    validation,
    roots,
    save,
    deletion,
    edit,
    submit,
  };
}
