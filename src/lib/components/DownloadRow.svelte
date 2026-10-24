<script lang="ts">
  import { isRunningStatus, type DownloadEntry } from '../stores/downloads.svelte';
  import { formatSpeed } from '../utils/format';

  interface Props {
    entry: DownloadEntry;
    onShow: () => void;
    onPause: () => void;
    onResume: () => void;
    onCancel: () => void;
    /** Absents quand l'élément est déjà en tête (ou en fin) de file. */
    onMoveUp?: () => void;
    onMoveDown?: () => void;
  }

  let { entry, onShow, onPause, onResume, onCancel, onMoveUp, onMoveDown }: Props = $props();

  let isRunning = $derived(isRunningStatus(entry.status));
  let canReorder = $derived(entry.status === 'waiting' || entry.status === 'paused');
  let percentage = $derived(Math.min(Math.max(entry.progressPercentage, 0), 100));

  let qualityLabel = $derived.by(() => {
    const formatId = entry.params.formatId ?? 'best';
    if (formatId === 'best') return 'Auto';
    if (formatId === 'audio_only') return 'MP3';
    return formatId;
  });

  let stateLabel = $derived.by(() => {
    if (entry.cancelRequested) return 'Annulation…';
    if (isRunning && entry.pauseRequested) return 'Mise en pause…';
    switch (entry.status) {
      case 'waiting':
        return 'En attente';
      case 'paused':
        return `En pause · ${percentage.toFixed(0)} %`;
      case 'queued':
        return 'Préparation…';
      case 'processing':
        return 'Traitement…';
      default: {
        const speed = entry.speedBytesPerSec > 0 ? ` · ${formatSpeed(entry.speedBytesPerSec)}` : '';
        return `${percentage.toFixed(1)} %${speed}`;
      }
    }
  });
</script>

<li class="row" data-running={isRunning}>
  <span class="row-lamp" aria-hidden="true"></span>

  <button type="button" class="row-main" onclick={onShow} title="Afficher la fenêtre de suivi">
    <span class="row-title">{entry.title}</span>
    <span class="row-meta">{qualityLabel} · {stateLabel}</span>
    <span class="row-meter" aria-hidden="true">
      <span class="row-meter-fill" style:width={`${percentage}%`}></span>
    </span>
  </button>

  <div class="row-actions">
    {#if canReorder}
      <button
        type="button"
        class="icon-btn"
        onclick={onMoveUp}
        disabled={!onMoveUp}
        aria-label="Monter « {entry.title} » dans la file"
        title="Monter dans la file"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <polyline points="18 15 12 9 6 15" />
        </svg>
      </button>
      <button
        type="button"
        class="icon-btn"
        onclick={onMoveDown}
        disabled={!onMoveDown}
        aria-label="Descendre « {entry.title} » dans la file"
        title="Descendre dans la file"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>
    {/if}

    {#if entry.status === 'paused'}
      <button
        type="button"
        class="icon-btn"
        onclick={onResume}
        disabled={entry.cancelRequested}
        aria-label="Reprendre « {entry.title} »"
        title="Reprendre"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <polygon points="7 4 20 12 7 20 7 4" />
        </svg>
      </button>
    {:else}
      <button
        type="button"
        class="icon-btn"
        onclick={onPause}
        disabled={entry.pauseRequested || entry.cancelRequested}
        aria-label="Mettre en pause « {entry.title} »"
        title="Pause"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <rect x="6" y="5" width="4" height="14" rx="1" />
          <rect x="14" y="5" width="4" height="14" rx="1" />
        </svg>
      </button>
    {/if}

    <button
      type="button"
      class="icon-btn cancel"
      onclick={onCancel}
      disabled={entry.cancelRequested}
      aria-label="Annuler « {entry.title} »"
      title="Annuler"
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

<style>
  .row {
    --lamp-color: var(--color-border-strong);
    --meter-color: var(--color-border-strong);

    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-top: 1px solid var(--color-border);
  }

  .row[data-running='true'] {
    --lamp-color: var(--color-accent);
    --meter-color: var(--color-accent);
  }

  .row-lamp {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
    background-color: var(--lamp-color);
  }

  .row-main {
    display: grid;
    gap: 2px;
    min-width: 0;
    flex: 1;
    padding: var(--space-1);
    border-radius: var(--radius-md);
    text-align: left;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .row-main:hover {
    background-color: var(--color-surface-hover);
  }

  .row-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-meta {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .row-meter {
    height: 3px;
    margin-top: 2px;
    border-radius: var(--radius-full);
    background-color: var(--color-surface-sunken);
    overflow: hidden;
  }

  .row-meter-fill {
    display: block;
    height: 100%;
    background-color: var(--meter-color);
    transition: width var(--duration-base) var(--ease-out);
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .icon-btn:hover:not(:disabled) {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .icon-btn.cancel:hover:not(:disabled) {
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
  }

  .icon-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
</style>
