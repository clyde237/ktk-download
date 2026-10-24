import { describe, expect, it } from 'vitest';
import {
  formatBytes,
  formatDuration,
  formatErrorMessage,
  formatFullDate,
  formatRelativeDate,
  formatSpeed
} from './format';

describe('format utils', () => {
  describe('formatErrorMessage', () => {
    it('returns default message for null or undefined', () => {
      expect(formatErrorMessage(null)).toBe('Une erreur inattendue est survenue.');
      expect(formatErrorMessage(undefined)).toBe('Une erreur inattendue est survenue.');
    });

    it('returns raw string when error is string', () => {
      expect(formatErrorMessage('Erreur réseau spécifique')).toBe('Erreur réseau spécifique');
    });

    it('extracts message property from error object', () => {
      expect(formatErrorMessage({ message: 'Fichier introuvable' })).toBe('Fichier introuvable');
    });

    it('returns fallback for unexpected object without message', () => {
      expect(formatErrorMessage({})).toBe(
        "Impossible de traiter la requête. Veuillez vérifier votre connexion et l'URL."
      );
    });
  });

  describe('formatBytes', () => {
    it('formats 0 or negative or invalid numbers as 0 B', () => {
      expect(formatBytes(0)).toBe('0 B');
      expect(formatBytes(-10)).toBe('0 B');
      expect(formatBytes(null)).toBe('0 B');
      expect(formatBytes(undefined)).toBe('0 B');
      expect(formatBytes(NaN)).toBe('0 B');
    });

    it('formats small byte counts correctly', () => {
      expect(formatBytes(500)).toBe('500 B');
    });

    it('formats kilobytes correctly', () => {
      expect(formatBytes(1024)).toBe('1.0 Ko');
      expect(formatBytes(15360)).toBe('15 Ko');
    });

    it('formats megabytes correctly', () => {
      expect(formatBytes(1048576)).toBe('1.0 Mo');
      expect(formatBytes(52428800)).toBe('50 Mo');
    });

    it('formats gigabytes correctly', () => {
      expect(formatBytes(1073741824)).toBe('1.0 Go');
      expect(formatBytes(3758096384)).toBe('3.5 Go');
    });
  });

  describe('formatSpeed', () => {
    it('formats 0 or negative speed as 0 Ko/s', () => {
      expect(formatSpeed(0)).toBe('0 Ko/s');
      expect(formatSpeed(null)).toBe('0 Ko/s');
      expect(formatSpeed(-5)).toBe('0 Ko/s');
    });

    it('formats active download speeds', () => {
      expect(formatSpeed(1048576)).toBe('1.0 Mo/s');
      expect(formatSpeed(512000)).toBe('500 Ko/s');
    });
  });

  describe('formatDuration', () => {
    it('returns placeholder for invalid durations', () => {
      expect(formatDuration(null)).toBe('--:--');
      expect(formatDuration(undefined)).toBe('--:--');
      expect(formatDuration(-1)).toBe('--:--');
      expect(formatDuration(NaN)).toBe('--:--');
    });

    it('formats minutes and seconds (mm:ss)', () => {
      expect(formatDuration(0)).toBe('00:00');
      expect(formatDuration(45)).toBe('00:45');
      expect(formatDuration(125)).toBe('02:05');
      expect(formatDuration(599)).toBe('09:59');
    });

    it('formats hours, minutes, and seconds (h:mm:ss)', () => {
      expect(formatDuration(3600)).toBe('1:00:00');
      expect(formatDuration(3665)).toBe('1:01:05');
      expect(formatDuration(7325)).toBe('2:02:05');
    });
  });

  describe('formatRelativeDate & formatFullDate', () => {
    it('handles invalid dates gracefully', () => {
      expect(formatRelativeDate('invalid-date')).toBe('');
      expect(formatFullDate('invalid-date')).toBe('');
    });

    it('formats recent date as immediate', () => {
      const now = new Date('2026-10-24T12:00:00Z');
      const recent = new Date('2026-10-24T11:59:45Z').toISOString();
      expect(formatRelativeDate(recent, now)).toBe("à l'instant");
    });

    it('formats relative minutes and hours', () => {
      const now = new Date('2026-10-24T12:00:00Z');
      const minutesAgo = new Date('2026-10-24T11:50:00Z').toISOString();
      expect(formatRelativeDate(minutesAgo, now)).toMatch(/minute/);

      const hoursAgo = new Date('2026-10-24T09:00:00Z').toISOString();
      expect(formatRelativeDate(hoursAgo, now)).toMatch(/heure/);
    });

    it('formats full date to locale string', () => {
      const valid = '2026-10-24T14:30:00Z';
      const formatted = formatFullDate(valid);
      expect(formatted).not.toBe('Date inconnue');
      expect(formatted.length).toBeGreaterThan(5);
    });
  });
});
