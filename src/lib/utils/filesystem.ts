import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import { toasts } from '../stores/toasts.svelte';
import { getFileName, getParentDirectory } from './path';

export { getFileName, getParentDirectory } from './path';

/**
 * Révèle un fichier téléchargé dans son dossier avec le gestionnaire de fichiers natif
 * (Explorateur Windows, Finder macOS, ou gestionnaire Linux tel que Nautilus / Dolphin).
 *
 * Si le fichier précis a été déplacé ou renommé, tente d'ouvrir le dossier parent à la place.
 * En cas d'échec complet, un toast explicatif est affiché à l'utilisateur.
 */
export async function revealItem(filePath: string, fallbackTitle?: string): Promise<boolean> {
  if (!filePath || filePath.trim() === '') {
    toasts.error({
      title: 'Chemin introuvable',
      message: 'Aucun emplacement de fichier associé à ce téléchargement.'
    });
    return false;
  }

  const displayName = fallbackTitle || getFileName(filePath);

  try {
    await revealItemInDir(filePath);
    return true;
  } catch (err: unknown) {
    console.warn('revealItemInDir a échoué, tentative d’ouverture du dossier parent :', err);

    const parentDir = getParentDirectory(filePath);
    if (parentDir && parentDir !== filePath) {
      try {
        await openPath(parentDir);
        toasts.info({
          title: 'Dossier ouvert',
          message: `« ${displayName} » semble déplacé ou supprimé. Le dossier a été ouvert.`
        });
        return true;
      } catch (parentErr: unknown) {
        console.warn('Ouverture du dossier parent impossible :', parentErr);
      }
    }

    toasts.error({
      title: 'Fichier introuvable',
      message: `« ${displayName} » a peut-être été déplacé ou supprimé.`
    });
    return false;
  }
}

/**
 * Ouvre directement un dossier dans le gestionnaire de fichiers du système.
 */
export async function openFolder(folderPath: string, folderLabel?: string): Promise<boolean> {
  if (!folderPath || folderPath.trim() === '') {
    toasts.error({
      title: 'Dossier non défini',
      message: 'Aucun dossier de destination n’a été sélectionné.'
    });
    return false;
  }

  const label = folderLabel || folderPath;

  try {
    await openPath(folderPath);
    return true;
  } catch (err: unknown) {
    console.warn('Ouverture du dossier impossible :', err);
    toasts.error({
      title: 'Dossier inaccessible',
      message: `Impossible d’ouvrir « ${label} » dans le gestionnaire de fichiers.`
    });
    return false;
  }
}

/**
 * Ouvre le fichier téléchargé avec l'application par défaut associée à son type MIME
 * (lecteur vidéo ou audio du système).
 */
export async function openMediaFile(filePath: string, fallbackTitle?: string): Promise<boolean> {
  if (!filePath || filePath.trim() === '') {
    return false;
  }

  const displayName = fallbackTitle || getFileName(filePath);

  try {
    await openPath(filePath);
    return true;
  } catch (err: unknown) {
    console.warn('Ouverture directe du fichier impossible :', err);
    toasts.error({
      title: 'Lecture impossible',
      message: `Impossible de lancer « ${displayName} ». Vérifiez qu'un lecteur compatible est installé.`
    });
    return false;
  }
}
