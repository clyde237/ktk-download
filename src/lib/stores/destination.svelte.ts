import { downloadDir, homeDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';

const SELECTED_FOLDER_KEY = 'ktk-download:destination';
const RECENT_FOLDERS_KEY = 'ktk-download:recent-folders';
const MAX_RECENT_FOLDERS = 5;

// Ces préférences sont un confort : si le stockage local est indisponible, l'application
// fonctionne avec le dossier par défaut, sans mémoriser les choix.
function readStorage(key: string): string | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string | null) {
  if (typeof localStorage === 'undefined') return;
  try {
    if (value === null) {
      localStorage.removeItem(key);
    } else {
      localStorage.setItem(key, value);
    }
  } catch (err: unknown) {
    console.warn('Préférence de destination non enregistrée :', err);
  }
}

function loadRecentFolders(): string[] {
  const raw = readStorage(RECENT_FOLDERS_KEY);
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((p): p is string => typeof p === 'string').slice(0, MAX_RECENT_FOLDERS)
      : [];
  } catch {
    return [];
  }
}

/**
 * Dossier de destination des téléchargements et accès rapide aux dossiers récents.
 *
 * `selected` à `null` signifie « dossier Téléchargements du système », résolu par le backend.
 * Le backend revérifie tout chemin reçu : ce store ne fait que mémoriser des préférences.
 */
class DestinationStore {
  selected = $state<string | null>(readStorage(SELECTED_FOLDER_KEY));
  recent = $state<string[]>(loadRecentFolders());
  defaultFolder = $state<string | null>(null);
  #homeFolder = $state<string | null>(null);
  #resolving: Promise<void> | null = null;

  /** Résout les dossiers système (Téléchargements, dossier personnel) pour l'affichage. */
  resolveSystemFolders(): Promise<void> {
    this.#resolving ??= Promise.all([downloadDir(), homeDir()])
      .then(([downloads, home]) => {
        this.defaultFolder = downloads;
        this.#homeFolder = home;
      })
      .catch((err: unknown) => {
        console.warn('Dossiers système non résolus :', err);
      });
    return this.#resolving;
  }

  select(folder: string | null) {
    this.selected = folder;
    writeStorage(SELECTED_FOLDER_KEY, folder);
  }

  /** Ouvre le sélecteur de dossier natif. Renvoie `false` si l'utilisateur annule. */
  async browse(): Promise<boolean> {
    const picked = await open({
      directory: true,
      multiple: false,
      title: 'Choisir le dossier de destination',
      defaultPath: this.selected ?? this.defaultFolder ?? undefined
    });
    if (typeof picked !== 'string') return false;
    this.select(picked);
    return true;
  }

  /** Mémorise un dossier réellement utilisé pour un téléchargement, en tête de liste. */
  remember(folder: string) {
    this.recent = [folder, ...this.recent.filter((f) => f !== folder)].slice(0, MAX_RECENT_FOLDERS);
    writeStorage(RECENT_FOLDERS_KEY, JSON.stringify(this.recent));
  }

  /** Raccourcit un chemin en remplaçant le dossier personnel par `~`. */
  displayPath(folder: string): string {
    const home = this.#homeFolder?.replace(/[\\/]+$/, '');
    if (
      home &&
      (folder === home || folder.startsWith(home + '/') || folder.startsWith(home + '\\'))
    ) {
      return '~' + folder.slice(home.length);
    }
    return folder;
  }
}

export const destination = new DestinationStore();
