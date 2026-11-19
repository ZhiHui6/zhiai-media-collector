import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CommandError,
  CoreEvent,
  HistoryEntry,
  QueueStats,
  Settings,
  TaskRecord,
} from "./types";

export interface Bootstrap {
  version: string;
  settings: Settings;
  history: HistoryEntry[];
  stats: QueueStats;
  tasks: TaskRecord[];
}

export interface LibraryFile {
  path: string;
  id: number;
  exists: boolean;
  width: number;
  height: number;
}

export interface DirectoryStatus {
  exists: boolean;
  writable: boolean;
}

const ERROR_TEXT: Record<string, string> = {
  invalid_path: "目录无效",
  invalid_input: "参数无效",
  storage: "配置保存失败",
  permission_denied: "没有写入权限",
  disk_full: "磁盘空间不足",
  file_busy: "文件被占用",
  file_access: "文件读写失败",
  verification: "需要访问验证",
  network_timeout: "网络超时",
  http_429: "请求过快",
};

function asCommandError(error: unknown): CommandError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const value = error as Partial<CommandError>;
    return {
      code: value.code ?? "unknown",
      message: String(value.message ?? ""),
      detail: value.detail ?? null,
    };
  }
  if (typeof error === "string") {
    return { code: "unknown", message: error, detail: null };
  }
  return { code: "unknown", message: String(error), detail: null };
}

/** 把命令错误转成可直接展示的中文文案，未知 code 回退到原始消息。 */
export function describeError(error: unknown): string {
  const value = asCommandError(error);
  return value.message || ERROR_TEXT[value.code] || "操作失败";
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
  resetSettings: () => invoke<Settings>("reset_settings"),
  setDownloadRoot: (path: string) => invoke<string>("set_download_root", { path }),
  checkDirectory: (path: string) => invoke<DirectoryStatus>("check_directory", { path }),
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