<script lang="ts">
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { history, type HistoryEntry } from '../stores/history.svelte';
  import { toasts } from '../stores/toasts.svelte';
  import { formatBytes, formatFullDate, formatRelativeDate } from '../utils/format';

  interface Props {
    onRetry: (entry: HistoryEntry) => void;
  }

  let { onRetry }: Props = $props();

  const COLLAPSED_COUNT = 6;
  // Délai pendant lequel « Effacer » attend une confirmation
  const CLEAR_CONFIRM_DELAY_MS = 4000;

  const STATUS_LABELS: Record<HistoryEntry['status'], string> = {
    completed: 'Terminé',
    failed: 'Échec',
    cancelled: 'Annulé'
  };

  let expanded = $state(false);
  let confirmingClear = $state(false);
  let clearTimer: ReturnType<typeof setTimeout> | null = null;

  let visibleEntries = $derived(
    expanded ? history.entries : history.entries.slice(0, COLLAPSED_COUNT)
  );
  let hiddenCount = $derived(history.entries.length - COLLAPSED_COUNT);

  function qualityLabel(formatId: string): string {
    if (formatId === 'best') return 'Auto';
    if (formatId === 'audio_only') return 'MP3';
    return formatId;
  }

  function metaLine(entry: HistoryEntry): string {
    const parts = [qualityLabel(entry.formatId)];
    if (entry.status === 'completed' && entry.fileSizeBytes) {
      parts.push(formatBytes(entry.fileSizeBytes));
    } else if (entry.status !== 'completed') {
      parts.push(STATUS_LABELS[entry.status]);
    }
    parts.push(formatRelativeDate(entry.finishedAt));
    return parts.join(' · ');
  }

  async function revealFile(entry: HistoryEntry) {
    if (!entry.filePath) return;
    try {
      await revealItemInDir(entry.filePath);
    } catch (err: unknown) {
      console.warn('Ouverture du dossier impossible :', err);
      toasts.error({
        title: 'Fichier introuvable',
        message: `« ${entry.title} » a peut-être été déplacé ou supprimé.`
      });
    }
  }

  function handleClear() {
    if (!confirmingClear) {
      confirmingClear = true;
      clearTimer = setTimeout(() => (confirmingClear = false), CLEAR_CONFIRM_DELAY_MS);
      return;
    }
    if (clearTimer) clearTimeout(clearTimer);
    confirmingClear = false;
    expanded = false;
    history.clear();
  }

  $effect(() => {
    return () => {
      if (clearTimer) clearTimeout(clearTimer);
    };
  });
</script>

<section class="history" aria-labelledby="history-heading">
  <header class="history-header">
    <h2 id="history-heading" class="history-heading">
      Historique <span class="history-count">{history.entries.length}</span>
    </h2>
    <button
      type="button"
      class="clear-history-btn"
      class:is-confirming={confirmingClear}
      onclick={handleClear}
    >
      {confirmingClear ? "Confirmer l'effacement" : 'Effacer'}
    </button>
  </header>

  <ol class="history-list">
    {#each visibleEntries as entry (entry.id)}
      <li class="history-item" data-status={entry.status}>
        <span class="item-lamp" aria-hidden="true"></span>

        <div class="item-body">
          <p class="item-title" title={entry.title}>{entry.title}</p>
          <p class="item-meta" title={formatFullDate(entry.finishedAt)}>
            <span class="visually-hidden">{STATUS_LABELS[entry.status]} — </span>{metaLine(entry)}
          </p>
          {#if entry.status === 'failed' && entry.errorMessage}
            <p class="item-error" title={entry.errorMessage}>{entry.errorMessage}</p>
          {/if}
        </div>

        <div class="item-actions">
          {#if entry.status === 'completed' && entry.filePath}
            <button type="button" class="item-btn" onclick={() => revealFile(entry)}>
              Ouvrir le dossier<span class="visually-hidden"> de « {entry.title} »</span>
            </button>
          {/if}
          <button type="button" class="item-btn" onclick={() => onRetry(entry)}>
            Relancer<span class="visually-hidden"> « {entry.title} »</span>
          </button>
          <button
            type="button"
            class="item-remove"
            onclick={() => history.remove(entry.id)}
            aria-label="Retirer « {entry.title} » de l'historique"
            title="Retirer de l'historique"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </li>
    {/each}
  </ol>

  {#if hiddenCount > 0}
    <button
      type="button"
      class="toggle-btn"
      aria-expanded={expanded}
      onclick={() => (expanded = !expanded)}
    >
      {expanded ? 'Afficher moins' : `Afficher tout (${history.entries.length})`}
    </button>
  {/if}
</section>

<style>
  .history {
    padding: var(--space-4) var(--space-5) var(--space-3);
    background-color: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-panel);
  }

  .history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-2);
  }

  .history-heading {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-sm);
    font-weight: 650;
    color: var(--color-text);
  }

  .history-count {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    color: var(--color-text-muted);
  }

  .clear-history-btn {
    min-height: 28px;
    padding: 0 var(--space-2);
    border-radius: var(--radius-md);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-text-muted);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .clear-history-btn:hover {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .clear-history-btn.is-confirming {
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
  }

  .history-list {
    list-style: none;
  }

  .history-item {
    --lamp-color: var(--color-success-meter);

    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) 0;
    border-top: 1px solid var(--color-border);
  }

  .history-item[data-status='failed'] {
    --lamp-color: var(--color-danger);
  }

  .history-item[data-status='cancelled'] {
    --lamp-color: var(--color-border-strong);
  }

  .item-lamp {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
    background-color: var(--lamp-color);
  }

  .item-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .item-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-meta {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--color-text-muted);
  }

  .item-error {
    font-size: var(--text-xs);
    color: var(--color-danger);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-actions {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    flex-shrink: 0;
  }

  .item-btn {
    min-height: 28px;
    padding: 0 var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background-color: var(--color-surface-sunken);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .item-btn:hover:not(:disabled) {
    background-color: var(--color-surface-hover);
  }

  .item-btn:disabled {
    color: var(--color-text-muted);
    cursor: not-allowed;
  }

  .item-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    color: var(--color-text-subtle);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .item-remove:hover {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .toggle-btn {
    width: 100%;
    min-height: 32px;
    margin-top: var(--space-1);
    border-top: 1px solid var(--color-border);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-text-muted);
  }

  .toggle-btn:hover {
    color: var(--color-text);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 560px) {
    .history-item {
      flex-wrap: wrap;
    }

    .item-actions {
      width: 100%;
      padding-left: calc(8px + var(--space-3));
    }
  }
</style>
