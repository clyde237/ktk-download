/**
 * Erreurs retournées par le backend Rust Tauri.
 * Correspondance exacte avec l'enum `CommandError` dans `src-tauri/src/error.rs`.
 */
export type CommandErrorKind =
  | 'InvalidUrl'
  | 'UnsupportedPlatform'
  | 'NetworkError'
  | 'ExtractionFailed'
  | 'FileSystemError'
  | 'DownloadFailed'
  | 'DownloadCancelled'
  | 'Internal';

export interface CommandError {
  kind: CommandErrorKind;
  message: string;
}

/**
 * Format et qualité disponibles pour une vidéo inspectée.
 */
export interface VideoFormat {
  format_id: string;
  label: string;
  resolution?: string;
  extension: string;
  file_size_bytes?: number;
  has_video: boolean;
  has_audio: boolean;
  direct_url?: string;
}

/**
 * Métadonnées d'une vidéo analysée par la commande IPC `inspect_url`.
 */
export interface VideoMetadata {
  id: string;
  title: string;
  description?: string;
  author?: string;
  thumbnail_url?: string;
  duration_seconds?: number;
  source_platform: string;
  source_url: string;
  available_formats: VideoFormat[];
}

/**
 * Paramètres pour lancer un téléchargement via la commande IPC `download_video`.
 */
export interface DownloadRequest {
  url: string;
  format_id?: string;
  output_dir?: string;
  custom_filename?: string;
}

/**
 * État d'avancement d'un téléchargement.
 */
export type DownloadStatus =
  'queued' | 'downloading' | 'processing' | 'completed' | 'failed' | 'cancelled';

/**
 * Charge utile de l'événement Tauri `download-progress`.
 */
export interface DownloadProgressPayload {
  download_id: string;
  status: DownloadStatus;
  progress_percentage: number;
  downloaded_bytes: number;
  total_bytes?: number;
  speed_bytes_per_sec: number;
  eta_seconds?: number;
}

/**
 * Résultat d'un téléchargement terminé.
 */
export interface DownloadResult {
  download_id: string;
  file_path: string;
  file_size_bytes: number;
  title: string;
}

/**
 * Résultat de la validation et assainissement d'une URL via `validate_url`.
 */
export interface ValidatedUrlPayload {
  is_valid: boolean;
  canonical_url: string;
  domain: string;
  platform: string;
}
