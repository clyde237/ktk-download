import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  CommandError,
  DownloadProgressPayload,
  DownloadRequest,
  DownloadResult,
  DownloadStatus
} from '../types/download';
import { formatErrorMessage } from '../utils/format';

/**
 * État d'un téléchargement tel qu'affiché par l'interface.
 */
export interface DownloadEntry {
  id: string;
  title: string;
  status: DownloadStatus;
  progressPercentage: number;
  downloadedBytes: number;
  totalBytes?: number;
  speedBytesPerSec: number;
  etaSeconds?: number;
  filePath?: string;
  errorMessage?: string;
  cancelRequested: boolean;
}

const TERMINAL_STATUSES: ReadonlySet<DownloadStatus> = new Set([
  'completed',
  'failed',
  'cancelled'
]);

export function isTerminalStatus(status: DownloadStatus): boolean {
  return TERMINAL_STATUSES.has(status);
}

function isCancellationError(err: unknown): boolean {
  return (
    typeof err === 'object' &&
    err !== null &&
    (err as Partial<CommandError>).kind === 'DownloadCancelled'
  );
}

/**
 * Store réactif des téléchargements, alimenté par l'événement Tauri `download-progress`.
 *
 * Les événements pilotent la progression en direct ; le résultat de `download_video`
 * reste la source de vérité pour l'état final (chemin du fichier, message d'erreur),
 * car le moteur yt-dlp n'émet pas toujours d'événement d'échec.
 */
class DownloadStore {
  entries = $state<Record<string, DownloadEntry>>({});

  #listener: Promise<UnlistenFn> | null = null;

  #ensureListening(): Promise<UnlistenFn> {
    this.#listener ??= listen<DownloadProgressPayload>('download-progress', (event) => {
      this.#applyProgress(event.payload);
    }).catch((err: unknown) => {
      // Permet une nouvelle tentative au prochain lancement au lieu de rester bloqué en échec.
      this.#listener = null;
      throw err;
    });
    return this.#listener;
  }

  #applyProgress(payload: DownloadProgressPayload) {
    const entry = this.entries[payload.download_id];
    // Ignore les téléchargements inconnus et les événements tardifs après un état final.
    if (!entry || isTerminalStatus(entry.status)) return;

    entry.status = payload.status;
    entry.speedBytesPerSec = payload.speed_bytes_per_sec;
    entry.etaSeconds = payload.eta_seconds;

    // Les événements d'échec et d'annulation remettent la progression à zéro : on conserve la
    // dernière valeur connue pour montrer où le téléchargement s'est arrêté.
    if (payload.status === 'failed' || payload.status === 'cancelled') return;

    entry.progressPercentage = payload.progress_percentage;
    entry.downloadedBytes = payload.downloaded_bytes;
    entry.totalBytes = payload.total_bytes;
  }

  /**
   * Lance un téléchargement et renvoie immédiatement son identifiant.
   * L'écoute des événements est active avant l'appel IPC pour ne manquer aucune progression.
   * Sans `outputDir`, le backend utilise le dossier Téléchargements du système ; sans
   * `formatId`, la meilleure qualité disponible. `fileName` (le titre de la vidéo) est assaini
   * et rendu unique par le backend ; sans lui, le fichier est nommé `video_<identifiant>`.
   */
  async start(
    url: string,
    title: string,
    options: { outputDir?: string; formatId?: string; fileName?: string } = {}
  ): Promise<string> {
    await this.#ensureListening();

    const id = crypto.randomUUID();
    this.entries[id] = {
      id,
      title,
      status: 'queued',
      progressPercentage: 0,
      downloadedBytes: 0,
      speedBytesPerSec: 0,
      cancelRequested: false
    };

    const request: DownloadRequest = {
      url,
      download_id: id,
      output_dir: options.outputDir,
      format_id: options.formatId,
      custom_filename: options.fileName
    };

    invoke<DownloadResult>('download_video', { request })
      .then((result) => {
        const entry = this.entries[id];
        if (!entry) return;
        entry.status = 'completed';
        entry.progressPercentage = 100;
        entry.downloadedBytes = result.file_size_bytes;
        entry.totalBytes = result.file_size_bytes;
        entry.speedBytesPerSec = 0;
        entry.etaSeconds = 0;
        entry.filePath = result.file_path;
      })
      .catch((err: unknown) => {
        const entry = this.entries[id];
        if (!entry) return;
        entry.speedBytesPerSec = 0;
        entry.etaSeconds = undefined;
        if (isCancellationError(err)) {
          entry.status = 'cancelled';
        } else {
          entry.status = 'failed';
          entry.errorMessage = formatErrorMessage(err);
        }
      });

    return id;
  }

  async cancel(id: string): Promise<void> {
    const entry = this.entries[id];
    if (!entry || isTerminalStatus(entry.status) || entry.cancelRequested) return;

    entry.cancelRequested = true;
    try {
      // `false` : le backend ne connaît pas (ou plus) ce téléchargement, l'annulation n'a pas eu lieu.
      const accepted = await invoke<boolean>('cancel_download', { downloadId: id });
      if (!accepted) entry.cancelRequested = false;
    } catch (err: unknown) {
      entry.cancelRequested = false;
      console.error("Échec de la demande d'annulation :", err);
    }
  }

  /** Retire un téléchargement terminé de l'affichage. */
  dismiss(id: string) {
    const entry = this.entries[id];
    if (entry && isTerminalStatus(entry.status)) {
      delete this.entries[id];
    }
  }
}

export const downloads = new DownloadStore();
