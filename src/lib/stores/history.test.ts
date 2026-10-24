import { beforeEach, describe, expect, it } from 'vitest';
import { history, type HistoryEntry } from './history.svelte';

describe('history store', () => {
  beforeEach(() => {
    history.clear();
  });

  it('records a new completed entry with timestamps', () => {
    const entry: HistoryEntry = {
      id: 'dl-1',
      title: 'Mon Voyage',
      sourceUrl: 'https://example.com/voyage.mp4',
      formatId: '1080p',
      status: 'completed',
      filePath: '/downloads/Mon Voyage.mp4',
      fileSizeBytes: 104857600,
      finishedAt: new Date().toISOString()
    };

    history.record(entry);
    expect(history.entries).toHaveLength(1);
    expect(history.entries[0]).toEqual(entry);
  });

  it('places newest recorded items at the beginning', () => {
    const entry1: HistoryEntry = {
      id: 'dl-1',
      title: 'Vidéo 1',
      sourceUrl: 'https://example.com/1.mp4',
      formatId: 'best',
      status: 'completed',
      finishedAt: new Date().toISOString()
    };

    const entry2: HistoryEntry = {
      id: 'dl-2',
      title: 'Vidéo 2',
      sourceUrl: 'https://example.com/2.mp4',
      formatId: 'best',
      status: 'completed',
      finishedAt: new Date().toISOString()
    };

    history.record(entry1);
    history.record(entry2);

    expect(history.entries).toHaveLength(2);
    expect(history.entries[0].id).toBe('dl-2');
    expect(history.entries[1].id).toBe('dl-1');
  });

  it('deduplicates entry by id when updated', () => {
    const entry: HistoryEntry = {
      id: 'dl-repeat',
      title: 'Version 1',
      sourceUrl: 'https://example.com/vid.mp4',
      formatId: '720p',
      status: 'failed',
      finishedAt: new Date().toISOString()
    };

    history.record(entry);
    expect(history.entries).toHaveLength(1);

    const updatedEntry: HistoryEntry = {
      ...entry,
      title: 'Version 2',
      status: 'completed'
    };

    history.record(updatedEntry);
    expect(history.entries).toHaveLength(1);
    expect(history.entries[0].title).toBe('Version 2');
    expect(history.entries[0].status).toBe('completed');
  });

  it('removes an item by id', () => {
    history.record({
      id: 'dl-to-delete',
      title: 'Supprimer moi',
      sourceUrl: 'https://example.com/del.mp4',
      formatId: 'best',
      status: 'completed',
      finishedAt: new Date().toISOString()
    });
    expect(history.entries).toHaveLength(1);

    history.remove('dl-to-delete');
    expect(history.entries).toHaveLength(0);
  });

  it('clears all history entries', () => {
    history.record({
      id: 'a',
      title: 'A',
      sourceUrl: 'https://example.com/a.mp4',
      formatId: 'best',
      status: 'completed',
      finishedAt: new Date().toISOString()
    });
    history.record({
      id: 'b',
      title: 'B',
      sourceUrl: 'https://example.com/b.mp4',
      formatId: 'best',
      status: 'completed',
      finishedAt: new Date().toISOString()
    });
    expect(history.entries).toHaveLength(2);

    history.clear();
    expect(history.entries).toHaveLength(0);
  });
});
