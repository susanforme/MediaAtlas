import { useMutation, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { pickDirectory } from "../../api/directories";
import {
  libraryApi,
  type MediaKind,
  type RootInput,
  type RootView,
  type ScanTask,
} from "../../api/library";
import {
  directoryName,
  newRootInput,
  normalizeRootInput,
  rootInputFromView,
} from "./library-model";
import { useLibraryDirectories } from "./use-library-directories";

export function useRootEditor(
  adult: boolean,
  onScanStarted?: (
    rootId: string,
    taskId: string,
    kind: MediaKind | null,
  ) => void,
) {
  const { t } = useTranslation();
  const { roots, adultAvailable, adultVisible } = useLibraryDirectories();
  const client = useQueryClient();
  const [form, setForm] = useState<RootInput | null>(null);
  const [excludes, setExcludes] = useState("");
  const [remove, setRemove] = useState<RootView | null>(null);
  const [validation, setValidation] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [picking, setPicking] = useState(false);
  const [pickerError, setPickerError] = useState<unknown>();
  const [operation, setOperation] = useState<"save" | "scan" | null>(null);
  const [savedRoot, setSavedRoot] = useState<RootView | null>(null);
  const [completed, setCompleted] = useState<{
    root: RootView;
    task?: ScanTask;
  } | null>(null);
  const epoch = useRef(0);
  const nameEdited = useRef(false);
  const operationLock = useRef(false);
  const pickerLock = useRef(false);
  const permitted = useRef(true);
  permitted.current = form?.kind !== "adult_video" || adultAvailable;
  const hadAdultAccess = useRef(adultAvailable);
  useEffect(() => {
    if (hadAdultAccess.current && !adultAvailable) {
      if (form?.kind === "adult_video") {
        epoch.current += 1;
        operationLock.current = false;
        setOperation(null);
      }
      setForm((current) => (current?.kind === "adult_video" ? null : current));
      setCompleted((current) =>
        current?.root.kind === "adult_video" ? null : current,
      );
      setRemove((current) =>
        current?.kind === "adult_video" ? null : current,
      );
    }
    hadAdultAccess.current = adultAvailable;
  }, [adultAvailable, form?.kind]);
  useEffect(
    () => () => {
      epoch.current += 1;
    },
    [],
  );
  const save = useMutation({
    mutationFn: (input: RootInput) => libraryApi.saveRoot(input),
  });
  const scan = useMutation({
    mutationFn: (id: string) => libraryApi.sync(id),
  });
  const deletion = useMutation({
    mutationFn: (id: string) => libraryApi.removeRoot(id),
    onSuccess: () => {
      setRemove(null);
      void client.invalidateQueries();
    },
  });
  function closeEditor() {
    if (operationLock.current) return;
    epoch.current += 1;
    pickerLock.current = false;
    setPicking(false);
    setPickerError(undefined);
    setForm(null);
  }
  function edit(root?: RootView) {
    if (
      operationLock.current ||
      (root?.kind === "adult_video" && !adultAvailable)
    )
      return;
    epoch.current += 1;
    pickerLock.current = false;
    nameEdited.current = Boolean(root);
    setPicking(false);
    setPickerError(undefined);
    setValidation(false);
    setAdvanced(false);
    setSavedRoot(null);
    setCompleted(null);
    save.reset();
    scan.reset();
    const value = root
      ? rootInputFromView(root)
      : newRootInput(adult && adultVisible);
    setForm(value);
    setExcludes(value.exclude_patterns.join("\n"));
  }
  function changeName(name: string) {
    nameEdited.current = true;
    setForm((current) => (current ? { ...current, name } : current));
  }
  function changePath(path: string) {
    setForm((current) =>
      current
        ? {
            ...current,
            path,
            name: nameEdited.current ? current.name : directoryName(path),
          }
        : current,
    );
  }
  async function chooseDirectory() {
    if (
      !form ||
      operationLock.current ||
      pickerLock.current ||
      !permitted.current
    )
      return;
    const started = epoch.current;
    pickerLock.current = true;
    setPicking(true);
    setPickerError(undefined);
    try {
      const path = await pickDirectory({
        title: t("rootChooseDirectory"),
        ...(form.path ? { defaultPath: form.path } : {}),
      });
      if (started !== epoch.current || !permitted.current || path === null)
        return;
      changePath(path);
    } catch (failure) {
      if (started === epoch.current) setPickerError(failure);
    } finally {
      if (started === epoch.current) {
        pickerLock.current = false;
        setPicking(false);
      }
    }
  }
  function normalizedForm(): RootInput | null {
    if (!form?.name.trim() || !form.path.trim()) {
      setValidation(true);
      return null;
    }
    return normalizeRootInput(form, excludes);
  }
  const savedVersion = Boolean(
    form &&
      savedRoot &&
      JSON.stringify(normalizeRootInput(form, excludes)) ===
        JSON.stringify(rootInputFromView(savedRoot)),
  );
  async function persist(scanAfter: boolean) {
    if (operationLock.current || pickerLock.current || !permitted.current)
      return;
    const input = normalizedForm();
    if (!input) return;
    const started = epoch.current;
    operationLock.current = true;
    setOperation(scanAfter ? "scan" : "save");
    try {
      // A failed scan must never re-create a directory that was already saved.
      const root =
        savedVersion && savedRoot ? savedRoot : await save.mutateAsync(input);
      void client.invalidateQueries({
        queryKey: root.kind === "adult_video" ? ["adult", "roots"] : ["roots"],
      });
      if (started !== epoch.current || !permitted.current) return;
      setSavedRoot(root);
      setForm(rootInputFromView(root));
      setExcludes(root.exclude_patterns.join("\n"));
      if (scanAfter) {
        const task = await scan.mutateAsync(root.id);
        void client.invalidateQueries({
          queryKey:
            root.kind === "adult_video" ? ["adult", "scan-tasks"] : ["tasks"],
        });
        if (started !== epoch.current || !permitted.current) return;
        setCompleted({ root, task });
        setForm(null);
        onScanStarted?.(root.id, task.id, root.kind);
      } else {
        setCompleted({ root });
        setForm(null);
      }
    } catch {
      // Mutations expose only the repository's typed, redacted errors.
    } finally {
      if (started === epoch.current) {
        operationLock.current = false;
        setOperation(null);
      }
    }
  }
  function submit(event: FormEvent) {
    event.preventDefault();
    // The backend chooses the isolated workflow and rechecks any adult grant.
    void persist(Boolean(form?.enabled));
  }
  return {
    form,
    setForm,
    excludes,
    setExcludes,
    remove,
    setRemove,
    validation,
    advanced,
    setAdvanced,
    roots,
    adultAvailable,
    adultVisible,
    save,
    scan,
    deletion,
    edit,
    submit,
    persist,
    closeEditor,
    changeName,
    changePath,
    chooseDirectory,
    picking,
    pickerError,
    operation,
    savedRoot,
    savedVersion,
    completed,
  };
}
