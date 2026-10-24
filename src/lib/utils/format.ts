/**
 * Utilitaires de formatage et d'affichage pour ktk-download.
 */

import type { CommandError } from '../types/download';

/**
 * Extrait un message d'erreur lisible pour l'utilisateur à partir d'une erreur backend ou frontend.
 */
export function formatErrorMessage(err: unknown): string {
  if (!err) {
    return 'Une erreur inattendue est survenue.';
  }

  if (typeof err === 'string') {
    return err;
  }

  if (typeof err === 'object') {
    const errorObj = err as Partial<CommandError> & { message?: string };
    if (errorObj.message && typeof errorObj.message === 'string') {
      return errorObj.message;
    }
  }

  return "Impossible de traiter la requête. Veuillez vérifier votre connexion et l'URL.";
}

/**
 * Formate un nombre d'octets en chaîne lisible (Ko, Mo, Go).
 */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || isNaN(bytes) || bytes < 0) {
    return '0 B';
  }

  if (bytes === 0) return '0 B';

  const units = ['B', 'Ko', 'Mo', 'Go', 'To'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const unitIndex = Math.min(i, units.length - 1);
  const value = bytes / Math.pow(k, unitIndex);

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/**
 * Formate un débit réseau en octets par seconde (Ko/s, Mo/s).
 */
export function formatSpeed(bytesPerSec: number | null | undefined): string {
  if (!bytesPerSec || bytesPerSec <= 0) {
    return '0 Ko/s';
  }
  return `${formatBytes(bytesPerSec)}/s`;
}

/**
 * Formate une durée en secondes au format mm:ss ou hh:mm:ss.
 */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || isNaN(seconds) || seconds < 0) {
    return '--:--';
  }

  const totalSecs = Math.round(seconds);
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  return `${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

const RELATIVE_TIME = new Intl.RelativeTimeFormat('fr', { numeric: 'auto' });
const ABSOLUTE_DATE = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'medium', timeStyle: 'short' });

/**
 * Formate une date passée de façon relative (« il y a 5 minutes », « hier »), puis en date
 * absolue au-delà d'une semaine.
 */
export function formatRelativeDate(isoDate: string, now: Date = new Date()): string {
  const date = new Date(isoDate);
  if (isNaN(date.getTime())) return '';

  const seconds = Math.round((date.getTime() - now.getTime()) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 60) return "à l'instant";
  if (abs < 3600) return RELATIVE_TIME.format(Math.round(seconds / 60), 'minute');
  if (abs < 86400) return RELATIVE_TIME.format(Math.round(seconds / 3600), 'hour');
  if (abs < 7 * 86400) return RELATIVE_TIME.format(Math.round(seconds / 86400), 'day');
  return ABSOLUTE_DATE.format(date);
}

/** Date complète, pour une info-bulle. */
export function formatFullDate(isoDate: string): string {
  const date = new Date(isoDate);
  return isNaN(date.getTime()) ? '' : ABSOLUTE_DATE.format(date);
}
