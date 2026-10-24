import { describe, expect, it } from 'vitest';
import {
  downloads,
  isRunningStatus,
  isTerminalStatus,
  MAX_CONCURRENT_DOWNLOADS,
  type DownloadEntry
} from './downloads.svelte';

describe('downloads store & helpers', () => {
  it('correctly identifies terminal statuses', () => {
    expect(isTerminalStatus('completed')).toBe(true);
    expect(isTerminalStatus('failed')).toBe(true);
    expect(isTerminalStatus('cancelled')).toBe(true);

    expect(isTerminalStatus('waiting')).toBe(false);
    expect(isTerminalStatus('queued')).toBe(false);
    expect(isTerminalStatus('downloading')).toBe(false);
    expect(isTerminalStatus('paused')).toBe(false);
  });

  it('correctly identifies running statuses', () => {
    expect(isRunningStatus('queued')).toBe(true);
    expect(isRunningStatus('downloading')).toBe(true);
    expect(isRunningStatus('processing')).toBe(true);

    expect(isRunningStatus('waiting')).toBe(false);
    expect(isRunningStatus('paused')).toBe(false);
    expect(isRunningStatus('completed')).toBe(false);
    expect(isRunningStatus('failed')).toBe(false);
    expect(isRunningStatus('cancelled')).toBe(false);
  });

  it('exposes maximum concurrent downloads limit of 2', () => {
    expect(MAX_CONCURRENT_DOWNLOADS).toBe(2);
  });

  describe('queue reordering & dismissal', () => {
    it('moves an entry up and down in queue priority', () => {
      // Nettoyage préalable
      downloads.entries = {};
      downloads.order = [];

      const makeEntry = (id: string, title: string): DownloadEntry => ({
        id,
        title,
        params: { url: `https://example.com/${id}.mp4` },
        status: 'waiting',
        progressPercentage: 0,
        downloadedBytes: 0,
        speedBytesPerSec: 0,
        cancelRequested: false,
        pauseRequested: false
      });

      downloads.entries = {
        'id-1': makeEntry('id-1', 'Vidéo 1'),
        'id-2': makeEntry('id-2', 'Vidéo 2'),
        'id-3': makeEntry('id-3', 'Vidéo 3')
      };
      downloads.order = ['id-1', 'id-2', 'id-3'];

      expect(downloads.ordered.map((e) => e.id)).toEqual(['id-1', 'id-2', 'id-3']);

      // Monter 'id-2' en tête
      downloads.move('id-2', -1);
      expect(downloads.order).toEqual(['id-2', 'id-1', 'id-3']);

      // Descendre 'id-2' en bas
      downloads.move('id-2', 1);
      expect(downloads.order).toEqual(['id-1', 'id-2', 'id-3']);

      // Déplacement hors bornes ignoré
      downloads.move('id-1', -1);
      expect(downloads.order).toEqual(['id-1', 'id-2', 'id-3']);
    });

    it('dismisses a single finished entry', () => {
      downloads.entries = {
        'id-done': {
          id: 'id-done',
          title: 'Fini',
          params: { url: 'https://example.com/done.mp4' },
          status: 'completed',
          progressPercentage: 100,
          downloadedBytes: 1000,
          speedBytesPerSec: 0,
          cancelRequested: false,
          pauseRequested: false
        },
        'id-active': {
          id: 'id-active',
          title: 'En cours',
          params: { url: 'https://example.com/active.mp4' },
          status: 'downloading',
          progressPercentage: 50,
          downloadedBytes: 500,
          speedBytesPerSec: 100,
          cancelRequested: false,
          pauseRequested: false
        }
      };
      downloads.order = ['id-done', 'id-active'];

      downloads.dismiss('id-done');
      expect(downloads.order).toEqual(['id-active']);
      expect(downloads.entries['id-done']).toBeUndefined();

      // Tentative de supprimer un élément actif : ignoré
      downloads.dismiss('id-active');
      expect(downloads.order).toEqual(['id-active']);
    });

    it('dismisses all finished entries with dismissFinished', () => {
      downloads.entries = {
        c1: {
          id: 'c1',
          title: 'Completed 1',
          params: { url: 'https://example.com/1' },
          status: 'completed',
          progressPercentage: 100,
          downloadedBytes: 100,
          speedBytesPerSec: 0,
          cancelRequested: false,
          pauseRequested: false
        },
        f1: {
          id: 'f1',
          title: 'Failed 1',
          params: { url: 'https://example.com/2' },
          status: 'failed',
          progressPercentage: 0,
          downloadedBytes: 0,
          speedBytesPerSec: 0,
          cancelRequested: false,
          pauseRequested: false
        },
        w1: {
          id: 'w1',
          title: 'Waiting 1',
          params: { url: 'https://example.com/3' },
          status: 'waiting',
          progressPercentage: 0,
          downloadedBytes: 0,
          speedBytesPerSec: 0,
          cancelRequested: false,
          pauseRequested: false
        }
      };
      downloads.order = ['c1', 'f1', 'w1'];

      downloads.dismissFinished();
      expect(downloads.order).toEqual(['w1']);
      expect(downloads.entries['c1']).toBeUndefined();
      expect(downloads.entries['f1']).toBeUndefined();
      expect(downloads.entries['w1']).toBeDefined();
    });
  });
});
