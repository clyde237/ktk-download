import { beforeEach, describe, expect, it } from 'vitest';
import { toasts } from './toasts.svelte';

describe('toasts store', () => {
  beforeEach(() => {
    // Nettoyer tous les toasts
    for (const item of [...toasts.items]) {
      toasts.dismiss(item.id);
    }
  });

  it('adds an info toast with correct defaults', () => {
    const id = toasts.info({ title: 'Téléchargement lancé', message: 'Vidéo test' });
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({
      id,
      kind: 'info',
      title: 'Téléchargement lancé',
      message: 'Vidéo test',
      duration: 4000
    });
  });

  it('adds an error toast without auto-dismiss duration', () => {
    const id = toasts.error({ title: 'Échec', message: 'Erreur réseau' });
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({
      id,
      kind: 'error',
      title: 'Échec',
      message: 'Erreur réseau',
      duration: null
    });
  });

  it('replaces existing toast with identical key instead of stacking', () => {
    toasts.info({ title: 'Étape 1', message: 'Préparation', key: 'task-1' });
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Étape 1');

    toasts.info({ title: 'Étape 2', message: 'Téléchargement', key: 'task-1' });
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].title).toBe('Étape 2');
    expect(toasts.items[0].message).toBe('Téléchargement');
  });

  it('dismisses toast by id', () => {
    const id1 = toasts.info({ title: 'A' });
    const id2 = toasts.success({ title: 'B' });
    expect(toasts.items).toHaveLength(2);

    toasts.dismiss(id1);
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].id).toBe(id2);
  });

  it('limits visible toasts to MAX_VISIBLE_TOASTS (4), trimming oldest non-error first', () => {
    toasts.info({ title: 'Info 1' });
    toasts.error({ title: 'Error 1' });
    toasts.info({ title: 'Info 2' });
    toasts.info({ title: 'Info 3' });
    expect(toasts.items).toHaveLength(4);

    // Ajout d'un 5ème toast : doit retirer le premier non-error (Info 1)
    toasts.info({ title: 'Info 4' });
    expect(toasts.items).toHaveLength(4);
    const titles = toasts.items.map((t) => t.title);
    expect(titles).not.toContain('Info 1');
    expect(titles).toContain('Error 1');
    expect(titles).toContain('Info 4');
  });
});
