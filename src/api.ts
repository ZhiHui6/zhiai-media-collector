import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CoreEvent, HistoryEntry, QueueStats, Settings } from "./types";

export interface Bootstrap {
  version: string;
  settings: Settings;
  history: HistoryEntry[];
  stats: QueueStats;
}

export interface LibraryFile {
  path: string;
  id: number;
  exists: boolean;
  width: number;
  height: number;
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  parseText: (text: string) => invoke<void>("parse_text", { text }),
  parseAlbum: (links: string[]) => invoke<void>("parse_album", { links }),
  enqueue: (ids: number[]) => invoke<void>("enqueue", { ids }),
  openPreview: (id: number) => invoke<void>("open_preview", { id }),
  cancelItem: (id: number) => invoke<void>("cancel_item", { id }),
  retryItem: (id: number) => invoke<void>("retry_item", { id }),
  retryAllFailed: () => invoke<void>("retry_all_failed"),
  pauseAll: () => invoke<void>("pause_all"),
  resumeAll: () => invoke<void>("resume_all"),
  setConcurrency: (value: number) => invoke<void>("set_concurrency", { value }),
  clearFinished: () => invoke<void>("clear_finished"),
  updateSettings: (settings: Settings) => invoke<void>("update_settings", { settings }),
  setDownloadRoot: (path: string) => invoke<string>("set_download_root", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openDirectory: (path: string) => invoke<void>("open_directory", { path }),
  applyTheme: (mode: string) => invoke<void>("apply_theme", { mode }),
  libraryPrepare: (paths: string[]) => invoke<LibraryFile[]>("library_prepare", { paths }),
  libraryDelete: (paths: string[], deleteFiles: boolean) =>
    invoke<number>("library_delete", { paths, deleteFiles }),
  mediaUrl: (kind: "thumb" | "full", id: number) => convertFileSrc(`${kind}/${id}`, "media"),
  libraryUrl: (id: number) => convertFileSrc(`lib/${id}`, "media"),
};

export function onCoreEvent(handler: (event: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>("core://event", (event) => handler(event.payload));
}