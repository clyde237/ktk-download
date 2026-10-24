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
import { history, type HistoryEntry } from './history.svelte';

/** Nombre maximal de téléchargements menés en parallèle. */
export const MAX_CONCURRENT_DOWNLOADS = 2;

/**
 * État d'un élément de la file. `waiting` n'existe que côté interface : l'élément attend
 * qu'un emplacement se libère et n'a pas encore été transmis au backend.
 */
export type EntryStatus = DownloadStatus | 'waiting';

/** Paramètres conservés pour lancer, et relancer après une pause, un téléchargement. */
export interface DownloadParams {
  url: string;
  formatId?: string;
  outputDir?: string;
  fileName?: string;
}

/**
 * État d'un téléchargement tel qu'affiché par l'interface.
 */
export interface DownloadEntry {
  id: string;
  title: string;
  params: DownloadParams;
  status: EntryStatus;
  progressPercentage: number;
  downloadedBytes: number;
  totalBytes?: number;
  speedBytesPerSec: number;
  etaSeconds?: number;
  filePath?: string;
  errorMessage?: string;
  cancelRequested: boolean;
  pauseRequested: boolean;
}

const TERMINAL_STATUSES: ReadonlySet<EntryStatus> = new Set(['completed', 'failed', 'cancelled']);
// États où le backend travaille sur le téléchargement : ils occupent un emplacement.
const RUNNING_STATUSES: ReadonlySet<EntryStatus> = new Set(['queued', 'downloading', 'processing']);

export function isTerminalStatus(status: EntryStatus): boolean {
  return TERMINAL_STATUSES.has(status);
}

export function isRunningStatus(status: EntryStatus): boolean {
  return RUNNING_STATUSES.has(status);
}

function errorKind(err: unknown): CommandError['kind'] | undefined {
  return typeof err === 'object' && err !== null ? (err as Partial<CommandError>).kind : undefined;
}

/**
 * File d'attente réactive des téléchargements.
 *
 * L'ordre de la file fixe la priorité : dès qu'un emplacement se libère, le premier élément en
 * attente démarre. Les événements `download-progress` pilotent la progression en direct ; le
 * résultat de `download_video` reste la source de vérité pour l'état final, car yt-dlp n'émet
 * pas toujours d'événement d'échec.
 */
class DownloadStore {
  entries = $state<Record<string, DownloadEntry>>({});
  /** Identifiants dans l'ordre de priorité de la file. */
  order = $state<string[]>([]);

  ordered = $derived(this.order.map((id) => this.entries[id]).filter((e) => e !== undefined));
  runningCount = $derived(this.ordered.filter((e) => isRunningStatus(e.status)).length);
  waitingCount = $derived(this.ordered.filter((e) => e.status === 'waiting').length);

  #listener: Promise<UnlistenFn> | null = null;

  #ensureListening(): Promise<UnlistenFn> {
    this.#listener ??= listen<DownloadProgressPayload>('download-progress', (event) => {
      this.#applyProgress(event.payload);
    }).catch((err: unknown) => {
      // Permet une nouvelle tentative au prochain ajout au lieu de rester bloqué en échec.
      this.#listener = null;
      throw err;
    });
    return this.#listener;
  }

  #applyProgress(payload: DownloadProgressPayload) {
    const entry = this.entries[payload.download_id];
    // Ignore les inconnus, et les événements tardifs une fois l'élément arrêté (fin ou pause).
    if (!entry || !isRunningStatus(entry.status)) return;

    entry.status = payload.status;
    entry.speedBytesPerSec = payload.speed_bytes_per_sec;
    entry.etaSeconds = payload.eta_seconds;

    // Échec, annulation et pause remettent la progression à zéro dans l'événement : on conserve
    // la dernière valeur connue pour montrer où le téléchargement s'est arrêté.
    if (
      payload.status === 'failed' ||
      payload.status === 'cancelled' ||
      payload.status === 'paused'
    ) {
      return;
    }

    entry.progressPercentage = payload.progress_percentage;
    entry.downloadedBytes = payload.downloaded_bytes;
    entry.totalBytes = payload.total_bytes;
  }

  /**
   * Ajoute un téléchargement en fin de file et renvoie son identifiant. Il démarre aussitôt si
   * un emplacement est libre. L'écoute des événements est active avant tout appel IPC.
   */
  async enqueue(title: string, params: DownloadParams): Promise<string> {
    await this.#ensureListening();

    const id = crypto.randomUUID();
    this.entries[id] = {
      id,
      title,
      params,
      status: 'waiting',
      progressPercentage: 0,
      downloadedBytes: 0,
      speedBytesPerSec: 0,
      cancelRequested: false,
      pauseRequested: false
    };
    this.order.push(id);
    this.#schedule();
    return id;
  }

  /** Démarre les éléments en attente, dans l'ordre de la file, tant qu'il reste de la place. */
  #schedule() {
    let running = this.runningCount;
    for (const id of this.order) {
      if (running >= MAX_CONCURRENT_DOWNLOADS) break;
      if (this.entries[id]?.status === 'waiting') {
        this.#launch(id);
        running++;
      }
    }
  }

  #launch(id: string) {
    const entry = this.entries[id];
    entry.status = 'queued';
    entry.cancelRequested = false;
    entry.pauseRequested = false;
    entry.errorMessage = undefined;
    entry.speedBytesPerSec = 0;

    const { title, params } = entry;
    const request: DownloadRequest = {
      url: params.url,
      download_id: id,
      output_dir: params.outputDir,
      format_id: params.formatId,
      custom_filename: params.fileName
    };

    invoke<DownloadResult>('download_video', { request })
      .then((result) => {
        this.#recordHistory(id, title, params, {
          status: 'completed',
          filePath: result.file_path,
          fileSizeBytes: result.file_size_bytes
        });
        const current = this.entries[id];
        if (current) {
          current.status = 'completed';
          current.progressPercentage = 100;
          current.downloadedBytes = result.file_size_bytes;
          current.totalBytes = result.file_size_bytes;
          current.speedBytesPerSec = 0;
          current.etaSeconds = 0;
          current.filePath = result.file_path;
        }
      })
      .catch((err: unknown) => {
        const kind = errorKind(err);
        const current = this.entries[id];

        if (kind === 'DownloadPaused') {
          // Pas d'historique : le téléchargement n'est pas terminé, il reprendra.
          if (current) {
            current.status = 'paused';
            current.pauseRequested = false;
            current.speedBytesPerSec = 0;
            current.etaSeconds = undefined;
          }
          return;
        }

        const cancelled = kind === 'DownloadCancelled';
        const errorMessage = cancelled ? undefined : formatErrorMessage(err);
        this.#recordHistory(
          id,
          title,
          params,
          cancelled ? { status: 'cancelled' } : { status: 'failed', errorMessage }
        );
        if (current) {
          current.status = cancelled ? 'cancelled' : 'failed';
          current.errorMessage = errorMessage;
          current.speedBytesPerSec = 0;
          current.etaSeconds = undefined;
        }
      })
      .finally(() => this.#schedule());
  }

  // Enregistré dans l'historique même si la carte a déjà été fermée entre-temps.
  #recordHistory(
    id: string,
    title: string,
    params: DownloadParams,
    outcome: Pick<HistoryEntry, 'status'> & Partial<HistoryEntry>
  ) {
    history.record({
      id,
      title,
      sourceUrl: params.url,
      formatId: params.formatId ?? 'best',
      outputDir: params.outputDir,
      finishedAt: new Date().toISOString(),
      ...outcome
    });
  }

  /**
   * Met en pause. En cours : le backend arrête le transfert en gardant les fichiers partiels.
   * En attente : l'élément est simplement retenu et ne démarrera pas.
   */
  async pause(id: string): Promise<void> {
    const entry = this.entries[id];
    if (!entry || entry.pauseRequested || entry.cancelRequested) return;

    if (entry.status === 'waiting') {
      entry.status = 'paused';
      return;
    }
    if (!isRunningStatus(entry.status)) return;

    entry.pauseRequested = true;
    try {
      // `false` : le backend ne connaît pas (ou plus) ce téléchargement, rien n'a été suspendu.
      const accepted = await invoke<boolean>('pause_download', { downloadId: id });
      if (!accepted) entry.pauseRequested = false;
    } catch (err: unknown) {
      entry.pauseRequested = false;
      console.error('Échec de la demande de pause :', err);
    }
  }

  /** Remet un élément en pause dans la file ; il reprendra dès qu'un emplacement se libère. */
  resume(id: string) {
    const entry = this.entries[id];
    if (entry?.status !== 'paused') return;
    entry.status = 'waiting';
    this.#schedule();
  }

  async cancel(id: string): Promise<void> {
    const entry = this.entries[id];
    if (!entry || isTerminalStatus(entry.status) || entry.cancelRequested) return;

    // Jamais transmis au backend : rien à arrêter ni à nettoyer, et rien à historiser.
    if (entry.status === 'waiting') {
      entry.status = 'cancelled';
      return;
    }

    entry.cancelRequested = true;
    const wasPaused = entry.status === 'paused';
    try {
      const accepted = await invoke<boolean>('cancel_download', { downloadId: id });
      if (wasPaused) {
        // Une pause ne laisse aucune commande en cours : l'annulation se conclut ici.
        entry.status = 'cancelled';
        if (accepted) {
          this.#recordHistory(id, entry.title, entry.params, { status: 'cancelled' });
        }
      } else if (!accepted) {
        entry.cancelRequested = false;
      }
    } catch (err: unknown) {
      entry.cancelRequested = false;
      console.error("Échec de la demande d'annulation :", err);
    }
  }

  /** Avance (`-1`) ou recule (`+1`) un élément d'une place dans la file. */
  move(id: string, offset: -1 | 1) {
    const index = this.order.indexOf(id);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= this.order.length) return;
    [this.order[index], this.order[target]] = [this.order[target], this.order[index]];
  }

  /** Retire un téléchargement terminé de la file. */
  dismiss(id: string) {
    const entry = this.entries[id];
    if (entry && isTerminalStatus(entry.status)) {
      delete this.entries[id];
      this.order = this.order.filter((o) => o !== id);
    }
  }

  /** Retire tous les téléchargements terminés de la file. */
  dismissFinished() {
    for (const entry of this.ordered) {
      if (isTerminalStatus(entry.status)) delete this.entries[entry.id];
    }
    this.order = this.order.filter((id) => this.entries[id] !== undefined);
  }
}

export const downloads = new DownloadStore();
