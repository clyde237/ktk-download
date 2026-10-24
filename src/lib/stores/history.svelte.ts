/**
 * Historique des téléchargements terminés (réussis, échoués ou annulés).
 *
 * Conservé dans le stockage local de la fenêtre : quelques dizaines d'entrées légères, sans
 * besoin de requêtes, ce qui ne justifie pas une base SQLite. Le format est versionné pour
 * pouvoir évoluer sans casser un historique existant.
 */

const HISTORY_KEY = 'ktk-download:history';
const HISTORY_VERSION = 1;
const MAX_HISTORY_ENTRIES = 100;

export type HistoryStatus = 'completed' | 'failed' | 'cancelled';

export interface HistoryEntry {
  id: string;
  title: string;
  sourceUrl: string;
  /** Préréglage de qualité demandé (`best`, `720p`, `audio_only`…). */
  formatId: string;
  /** Dossier choisi ; absent : dossier Téléchargements par défaut. */
  outputDir?: string;
  status: HistoryStatus;
  filePath?: string;
  fileSizeBytes?: number;
  errorMessage?: string;
  /** Date de fin au format ISO 8601. */
  finishedAt: string;
}

const HISTORY_STATUSES: ReadonlySet<string> = new Set(['completed', 'failed', 'cancelled']);

function isOptionalString(value: unknown): boolean {
  return value === undefined || typeof value === 'string';
}

/** Vérifie la forme d'une entrée relue : le stockage local peut contenir n'importe quoi. */
function isHistoryEntry(value: unknown): value is HistoryEntry {
  if (typeof value !== 'object' || value === null) return false;
  const entry = value as Record<string, unknown>;
  return (
    typeof entry.id === 'string' &&
    typeof entry.title === 'string' &&
    typeof entry.sourceUrl === 'string' &&
    typeof entry.formatId === 'string' &&
    typeof entry.status === 'string' &&
    HISTORY_STATUSES.has(entry.status) &&
    typeof entry.finishedAt === 'string' &&
    isOptionalString(entry.outputDir) &&
    isOptionalString(entry.filePath) &&
    isOptionalString(entry.errorMessage) &&
    (entry.fileSizeBytes === undefined || typeof entry.fileSizeBytes === 'number')
  );
}

function loadHistory(): HistoryEntry[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== 'object' ||
      parsed === null ||
      (parsed as { version?: unknown }).version !== HISTORY_VERSION
    ) {
      return [];
    }
    const entries = (parsed as { entries?: unknown }).entries;
    return Array.isArray(entries)
      ? entries.filter(isHistoryEntry).slice(0, MAX_HISTORY_ENTRIES)
      : [];
  } catch (err: unknown) {
    console.warn('Historique illisible, ignoré :', err);
    return [];
  }
}

class HistoryStore {
  /** Entrées de la plus récente à la plus ancienne. */
  entries = $state<HistoryEntry[]>(loadHistory());

  record(entry: HistoryEntry) {
    this.entries = [entry, ...this.entries.filter((e) => e.id !== entry.id)].slice(
      0,
      MAX_HISTORY_ENTRIES
    );
    this.#persist();
  }

  remove(id: string) {
    this.entries = this.entries.filter((e) => e.id !== id);
    this.#persist();
  }

  clear() {
    this.entries = [];
    this.#persist();
  }

  #persist() {
    try {
      localStorage.setItem(
        HISTORY_KEY,
        JSON.stringify({ version: HISTORY_VERSION, entries: $state.snapshot(this.entries) })
      );
    } catch (err: unknown) {
      // L'historique reste utilisable pendant la session, il ne survivra simplement pas au redémarrage.
      console.warn('Historique non enregistré :', err);
    }
  }
}

export const history = new HistoryStore();
