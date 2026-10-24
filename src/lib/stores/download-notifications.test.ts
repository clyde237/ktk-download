import { beforeEach, describe, expect, it } from 'vitest';
import { createDownloadNotifier } from './download-notifications';
import type { DownloadEntry } from './downloads.svelte';
import { toasts } from './toasts.svelte';

function createMockEntry(overrides: Partial<DownloadEntry> = {}): DownloadEntry {
  return {
    id: 'entry-1',
    title: 'Superbe Documentaire',
    params: { url: 'https://example.com/video.mp4', formatId: 'best' },
    status: 'waiting',
    progressPercentage: 0,
    downloadedBytes: 0,
    speedBytesPerSec: 0,
    cancelRequested: false,
    pauseRequested: false,
    ...overrides
  };
}

describe('download-notifications', () => {
  beforeEach(() => {
    for (const item of [...toasts.items]) {
      toasts.dismiss(item.id);
    }
  });

  it('notifies waiting in queue when status is waiting', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({ status: 'waiting' });

    notify(entry);

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe("Ajouté à la file d'attente");
    expect(toasts.items[0].message).toBe('Superbe Documentaire');
  });

  it('notifies download started when running', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({ status: 'downloading', progressPercentage: 5 });

    notify(entry);

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Téléchargement lancé');
  });

  it('notifies progress milestones at 25%, 50%, and 75% without duplicate spam', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({ status: 'downloading', progressPercentage: 10 });

    notify(entry); // Start
    expect(toasts.items[0].title).toBe('Téléchargement lancé');

    entry.progressPercentage = 24;
    notify(entry); // Below 25
    expect(toasts.items[0].title).toBe('Téléchargement lancé');

    entry.progressPercentage = 30;
    notify(entry); // Hit 25% milestone
    expect(toasts.items[0].title).toBe('25 % téléchargés');

    entry.progressPercentage = 35;
    notify(entry); // Still within 25% milestone
    expect(toasts.items[0].title).toBe('25 % téléchargés');

    entry.progressPercentage = 55;
    notify(entry); // Hit 50% milestone
    expect(toasts.items[0].title).toBe('50 % téléchargés');

    entry.progressPercentage = 80;
    notify(entry); // Hit 75% milestone
    expect(toasts.items[0].title).toBe('75 % téléchargés');
  });

  it('notifies completion with extracted filename', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({
      status: 'completed',
      filePath: '/home/user/Downloads/mon-film.mp4'
    });

    notify(entry);

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Téléchargement terminé');
    expect(toasts.items[0].message).toBe('mon-film.mp4');
    expect(toasts.items[0].action?.label).toBe('Ouvrir le dossier');
  });

  it('notifies failure with error message', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({
      status: 'failed',
      errorMessage: 'Espace disque insuffisant'
    });

    notify(entry);

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Échec du téléchargement');
    expect(toasts.items[0].message).toBe('Espace disque insuffisant');
  });

  it('notifies cancellation', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({
      status: 'cancelled'
    });

    notify(entry);

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Téléchargement annulé');
  });

  it('stops notifying after outcome (completed/failed/cancelled)', () => {
    const notify = createDownloadNotifier();
    const entry = createMockEntry({
      status: 'completed',
      filePath: '/home/user/video.mp4'
    });

    notify(entry);
    expect(toasts.items[0].title).toBe('Téléchargement terminé');

    // Simulate extra late progress event
    entry.progressPercentage = 100;
    notify(entry);
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Téléchargement terminé');
  });
});
