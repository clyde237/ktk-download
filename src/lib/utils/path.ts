/**
 * Utilitaires purs de manipulation de chemins de fichiers cross-platform.
 */

/**
 * Extrait le dossier parent d'un chemin de fichier, en gérant les séparateurs Unix et Windows.
 */
export function getParentDirectory(filePath: string): string {
  const normalized = filePath.replace(/\\+/g, '/').replace(/\/+$/, '');
  const lastSlash = normalized.lastIndexOf('/');
  if (lastSlash <= 0) {
    return normalized;
  }
  return normalized.slice(0, lastSlash);
}

/**
 * Extrait le nom de base d'un fichier à partir de son chemin complet.
 */
export function getFileName(filePath: string): string {
  const normalized = filePath.replace(/\\+/g, '/').replace(/\/+$/, '');
  const lastSlash = normalized.lastIndexOf('/');
  return lastSlash >= 0 ? normalized.slice(lastSlash + 1) : normalized;
}
