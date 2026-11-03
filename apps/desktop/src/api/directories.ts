import { isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { DesktopError } from "./desktop";

export interface DirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

export async function pickDirectory({
  title,
  defaultPath,
}: DirectoryPickerOptions): Promise<string | null> {
  if (!isTauri()) throw new DesktopError("desktop_required");

  let selected: unknown;
  try {
    selected = await open({
      title,
      ...(defaultPath ? { defaultPath } : {}),
      directory: true,
      multiple: false,
      recursive: false,
    });
  } catch {
    // Native dialog errors may contain private filesystem paths.
    throw new DesktopError("request_failed");
  }
  if (selected === null) return null;
  if (typeof selected !== "string" || !selected.trim()) {
    throw new DesktopError("invalid_response");
  }
  return selected;
}
