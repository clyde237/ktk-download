/**
 * Échanges entre la fenêtre principale et les fenêtres de suivi des téléchargements.
 *
 * Chaque fenêtre Tauri a son propre contexte JavaScript : la file d'attente vit dans la fenêtre
 * principale, seule à lancer les commandes IPC. Les fenêtres de suivi en reçoivent l'état et lui
 * renvoient les actions de l'utilisateur, par des événements Tauri.
 */
import type { DownloadEntry } from './downloads.svelte';

/** Fenêtre principale → toutes les fenêtres : état à jour d'un téléchargement. */
export const ENTRY_EVENT = 'queue:entry';
/** Fenêtre de suivi → fenêtre principale : demande l'état actuel d'un téléchargement. */
export const ENTRY_REQUEST_EVENT = 'queue:entry-request';
/** Fenêtre de suivi → fenêtre principale : action de l'utilisateur sur un téléchargement. */
export const COMMAND_EVENT = 'queue:command';

export type QueueAction = 'pause' | 'resume' | 'cancel' | 'retry';

export interface EntryRequest {
  id: string;
}

export interface QueueCommand {
  id: string;
  action: QueueAction;
}

/** Réponse quand la fenêtre principale ne connaît pas (ou plus) le téléchargement demandé. */
export interface MissingEntry {
  id: string;
  missing: true;
}

export type EntryMessage = DownloadEntry | MissingEntry;

export function isMissingEntry(message: EntryMessage): message is MissingEntry {
  return 'missing' in message;
}
