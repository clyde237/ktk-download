<script lang="ts">
  import { page } from '$app/state';
  import { emit, listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import DownloadProgress from '$lib/components/DownloadProgress.svelte';
  import type { DownloadEntry } from '$lib/stores/downloads.svelte';
  import {
    COMMAND_EVENT,
    ENTRY_EVENT,
    ENTRY_REQUEST_EVENT,
    isMissingEntry,
    type EntryMessage,
    type EntryRequest,
    type QueueAction,
    type QueueCommand
  } from '$lib/stores/window-sync';
  import { getParentDirectory } from '$lib/utils/path';

  // Fenêtre de suivi d'un seul téléchargement, identifié dans l'URL (`/download?id=…`).
  const downloadId = page.url.searchParams.get('id');

  // Délai au-delà duquel la fenêtre principale est considérée comme ne connaissant pas ce téléchargement
  const ENTRY_WAIT_MS = 3000;

  let entry = $state<DownloadEntry | null>(null);
  let missing = $state(false);

  let qualityLabel = $derived.by(() => {
    const formatId = entry?.params.formatId ?? 'best';
    if (formatId === 'best') return 'Meilleure qualité';
    if (formatId === 'audio_only') return 'Audio MP3';
    return formatId;
  });

  let folderLabel = $derived(
    entry?.filePath
      ? getParentDirectory(entry.filePath)
      : (entry?.params.outputDir ?? 'Dossier Téléchargements')
  );

  function send(action: QueueAction) {
    if (!downloadId) return;
    void emit<QueueCommand>(COMMAND_EVENT, { id: downloadId, action });
  }

  function closeWindow() {
    void getCurrentWindow().close();
  }

  function retry() {
    send('retry');
    // La relance crée un nouveau téléchargement, suivi dans sa propre fenêtre.
    closeWindow();
  }

  onMount(() => {
    if (!downloadId) {
      missing = true;
      return;
    }

    let disposed = false;
    const unlistenPromise = listen<EntryMessage>(ENTRY_EVENT, (event) => {
      const message = event.payload;
      if (message.id !== downloadId) return;
      if (isMissingEntry(message)) {
        missing = entry === null;
      } else {
        entry = message;
        missing = false;
      }
    });

    // L'écoute doit être active avant la demande, sinon la réponse pourrait être perdue.
    void unlistenPromise.then(() => {
      if (!disposed) void emit<EntryRequest>(ENTRY_REQUEST_EVENT, { id: downloadId });
    });

    const timeout = setTimeout(() => {
      if (entry === null) missing = true;
    }, ENTRY_WAIT_MS);

    return () => {
      disposed = true;
      clearTimeout(timeout);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  });
</script>

<main class="popup">
  {#if entry}
    <DownloadProgress
      {entry}
      onPause={() => send('pause')}
      onResume={() => send('resume')}
      onCancel={() => send('cancel')}
      onRetry={retry}
      onDismiss={closeWindow}
    />

    <dl class="details">
      <div class="detail">
        <dt>Qualité</dt>
        <dd>{qualityLabel}</dd>
      </div>
      <div class="detail">
        <dt>Dossier</dt>
        <dd title={folderLabel}>{folderLabel}</dd>
      </div>
    </dl>

    <p class="hint">Fermer cette fenêtre n'arrête pas le téléchargement.</p>
  {:else if missing}
    <section class="missing" role="alert">
      <h1 class="missing-title">Téléchargement introuvable</h1>
      <p class="missing-text">
        Ce téléchargement n'est plus suivi par ktk-download. Retrouvez-le dans l'onglet « Terminés »
        de la fenêtre principale.
      </p>
      <button type="button" class="close-btn" onclick={closeWindow}>Fermer</button>
    </section>
  {:else}
    <p class="loading" aria-live="polite">Chargement…</p>
  {/if}
</main>

<style>
  .popup {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-height: 100vh;
    padding: var(--space-4);
    background-color: var(--color-bg);
  }

  .details {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-4);
    padding: 0 var(--space-1);
  }

  .detail {
    display: contents;
  }

  .details dt {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
    line-height: 1.8;
  }

  .details dd {
    min-width: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-text);
    line-height: 1.8;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }

  .hint {
    margin-top: auto;
    padding: 0 var(--space-1);
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }

  .missing {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-5);
    background-color: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
  }

  .missing-title {
    font-size: var(--text-base);
    font-weight: 650;
  }

  .missing-text {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
  }

  .close-btn {
    align-self: flex-end;
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background-color: var(--color-surface-sunken);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .loading {
    margin: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }
</style>
