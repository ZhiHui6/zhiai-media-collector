export type ImageKind = "Article" | "Cover" | "Avatar" | "QrCode" | "Album" | "Video" | "Unknown";
export type ItemState = "Queued" | "Downloading" | "Completed" | "Failed" | "Cancelled" | "Skipped";
export type PageKind = "Article" | "PicturePost" | "AlbumPage" | "Unknown";
export type ThemeMode = "System" | "Light" | "Dark";
export type ConvertTarget = "Jpg" | "Png" | "WebP";
export type DuplicatePolicy = "Skip" | "AppendSuffix" | "Overwrite";

export interface ImageItem {
  id: number;
  url: string;
  kind: ImageKind;
  seq: number;
  page_url: string;
  page_title: string;
  format_hint: string | null;
  selected: boolean;
}

export interface ParseBatch {
  page_url: string;
  page_title: string;
  account_name: string;
  page_kind: PageKind;
  items: ImageItem[];
  warnings: string[];
}

export type ParseOutcome =
  | { Images: ParseBatch }
  | { ArticleLinks: { page_url: string; title: string; links: string[] } }
  | { Failed: { url: string; message: string } };

export interface QueueStats {
  queued: number;
  active: number;
  completed: number;
  failed: number;
  paused: boolean;
  concurrency: number;
}

export interface Settings {
  schema_version: number;
  theme: ThemeMode;
  download_root: string;
  concurrency: number;
  retry_max: number;
  convert_enabled: boolean;
  convert_target: ConvertTarget;
  jpeg_quality: number;
  webp_quality: number;
  duplicate_policy: DuplicatePolicy;
  /** KB/s，0 表示不限速。 */
  speed_limit_kbps: number;
  /** 批量解析相邻请求的最小间隔（毫秒）。 */
  parse_interval_ms: number;
}

export interface TaskRecord {
  item: ImageItem;
  state: ItemState;
  attempts: number;
  message: string | null;
  path: string | null;
}

export interface CommandError {
  code: string;
  message: string;
  detail?: string | null;
}

export interface HistoryEntry {
  page_url: string;
  page_title: string;
  image_url: string;
  kind: ImageKind;
  path: string;
  bytes: number;
  format: string;
  finished_at: string;
}

export type CoreEvent =
  | { type: "parseStarted"; url: string }
  | { type: "batchParsed"; outcome: ParseOutcome }
  | {
      type: "itemState";
      id: number;
      state: ItemState;
      message: string | null;
      action: string | null;
    }
  | { type: "itemProgress"; id: number; done: number; total: number }
  | { type: "itemCompleted"; id: number; path: string; bytes: number; format: string }
  | { type: "thumbReady"; id: number; width: number; height: number }
  | { type: "fullReady"; id: number; width: number; height: number }
  | { type: "fullFailed"; id: number; message: string }
  | { type: "queueStats"; stats: QueueStats }
  | { type: "history"; entry: HistoryEntry };

export const KIND_LABEL: Record<ImageKind, string> = {
  Article: "文章配图",
  Cover: "封面",
  Avatar: "头像",
  QrCode: "二维码",
  Album: "相册图片",
  Video: "视频",
  Unknown: "其他",
};

export const STATE_LABEL: Record<ItemState, string> = {
  Queued: "等待中",
  Downloading: "下载中",
  Completed: "已完成",
  Failed: "失败",
  Cancelled: "已取消",
  Skipped: "已跳过",
};

export function stateColor(state: ItemState): string {
  switch (state) {
    case "Completed":
      return "var(--brand)";
    case "Downloading":
      return "var(--blue)";
    case "Failed":
      return "var(--red)";
    case "Skipped":
      return "var(--orange)";
    default:
      return "var(--faint)";
  }
}

export function kindTag(kind: ImageKind): string {
  switch (kind) {
    case "Article":
      return "blue";
    case "Cover":
      return "purple";
    case "Avatar":
      return "orange";
    case "QrCode":
      return "green";
    case "Album":
      return "link";
    case "Video":
      return "video";
    default:
      return "gray";
  }
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

export function formatSpeed(bytes: number): string {
  return `${formatBytes(Math.max(0, bytes))}/s`;
}

export function progressPercent(done: number, total: number): number {
  if (!total) return 0;
  return Math.min(100, Math.round((done / total) * 100));
}