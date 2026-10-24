<script lang="ts">
  import {
    isRunningStatus,
    isTerminalStatus,
    type DownloadEntry
  } from '../stores/downloads.svelte';
  import { revealItem } from '../utils/filesystem';
  import { formatBytes, formatDuration, formatSpeed } from '../utils/format';

  interface Props {
    entry: DownloadEntry;
    onCancel?: () => void;
    onDismiss?: () => void;
    onPause?: () => void;
    onResume?: () => void;
    onRetry?: () => void;
    /** Absents quand l'élément est déjà en tête (ou en fin) de file. */
    onMoveUp?: () => void;
    onMoveDown?: () => void;
  }

  let { entry, onCancel, onDismiss, onPause, onResume, onRetry, onMoveUp, onMoveDown }: Props =
    $props();

  let isActive = $derived(!isTerminalStatus(entry.status));
  let isRunning = $derived(isRunningStatus(entry.status));
  // L'ordre ne compte que pour ce qui n'a pas encore démarré ou reprendra plus tard.
  let canReorder = $derived(entry.status === 'waiting' || entry.status === 'paused');

  // yt-dlp reste à 100 % pendant la fusion audio/vidéo, avant l'événement `completed`.
  let isFinalizing = $derived(entry.status === 'downloading' && entry.progressPercentage >= 100);

  // Sans taille totale ni pourcentage, aucune progression mesurable : barre indéterminée.
  let isIndeterminate = $derived(
    isRunning &&
      (entry.status === 'queued' ||
        entry.status === 'processing' ||
        isFinalizing ||
        (entry.progressPercentage <= 0 && !entry.totalBytes))
  );

  let percentage = $derived(
    entry.status === 'completed' ? 100 : Math.min(Math.max(entry.progressPercentage, 0), 100)
  );

  let percentageLabel = $derived(`${percentage.toFixed(1)} %`);

  // Compteur affiché : tirets tant qu'aucune mesure n'est possible, comme un afficheur en veille.
  let hasNoMeasure = $derived(isIndeterminate || (entry.status === 'waiting' && percentage <= 0));
  let readout = $derived(hasNoMeasure ? '--.- %' : percentageLabel);

  let statusLabel = $derived.by(() => {
    if (isActive && entry.cancelRequested) return 'Annulation…';
    if (isRunning && entry.pauseRequested) return 'Mise en pause…';
    switch (entry.status) {
      case 'waiting':
        return "En file d'attente";
      case 'paused':
        return 'En pause';
      case 'queued':
        return 'Préparation…';
      case 'downloading':
        return isFinalizing ? 'Finalisation du fichier…' : 'Téléchargement en cours';
      case 'processing':
        return 'Traitement…';
      case 'completed':
        return 'Téléchargement terminé';
      case 'failed':
        return 'Échec du téléchargement';
      case 'cancelled':
        return 'Téléchargement annulé';
    }
  });

  let sizeLabel = $derived.by(() => {
    if (entry.totalBytes) {
      return `${formatBytes(entry.downloadedBytes)} / ${formatBytes(entry.totalBytes)}`;
    }
    return entry.downloadedBytes > 0 ? formatBytes(entry.downloadedBytes) : null;
  });

  let showLiveStats = $derived(entry.status === 'downloading' && !isFinalizing);

  let valueText = $derived(
    isIndeterminate ? statusLabel : sizeLabel ? `${percentageLabel}, ${sizeLabel}` : percentageLabel
  );

  async function handleReveal() {
    if (entry.filePath) {
      await revealItem(entry.filePath, entry.title);
    }
  }
</script>

<article
  class="download-card"
  class:is-completed={entry.status === 'completed'}
  class:is-failed={entry.status === 'failed'}
  class:is-cancelled={entry.status === 'cancelled'}
  class:is-on-hold={entry.status === 'waiting' || entry.status === 'paused'}
  aria-label="Téléchargement de {entry.title}"
>
  <header class="card-header">
    <p class="status-line">
      <span class="status-icon" aria-hidden="true">
        {#if entry.status === 'completed'}
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        {:else if entry.status === 'failed'}
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
        {:else if entry.status === 'cancelled'}
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />
          </svg>
        {:else if isRunning}
          <span class="rec-dot"></span>
        {:else}
          <span class="hold-dot"></span>
        {/if}
      </span>
      <span class="status-label">{statusLabel}</span>
    </p>

    <div class="card-actions">
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
        <button type="button" class="card-btn" onclick={onResume} disabled={entry.cancelRequested}>
          Reprendre
        </button>
      {:else if isActive}
        <button
          type="button"
          class="card-btn"
          onclick={onPause}
          disabled={entry.pauseRequested || entry.cancelRequested}
        >
          Pause
        </button>
      {/if}

      {#if isActive}
        <button
          type="button"
          class="card-btn cancel-btn"
          onclick={onCancel}
          disabled={entry.cancelRequested}
        >
          Annuler
        </button>
      {:else}
        {#if entry.status === 'completed' && entry.filePath}
          <button
            type="button"
            class="card-btn open-btn"
            onclick={handleReveal}
            title="Afficher le fichier dans le dossier de destination"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path
                d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
              />
            </svg>
            <span>Ouvrir le dossier</span>
          </button>
        {/if}
        {#if entry.status === 'failed' && onRetry}
          <button
            type="button"
            class="card-btn retry-btn"
            onclick={onRetry}
            title="Relancer le téléchargement"
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
              <path d="M21 2v6h-6" />
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
              <path d="M3 22v-6h6" />
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            </svg>
            <span>Réessayer</span>
          </button>
        {/if}
        <button type="button" class="card-btn dismiss-btn" onclick={onDismiss}>Fermer</button>
      {/if}
    </div>
  </header>

  <p class="download-title" title={entry.title}>{entry.title}</p>

  <div class="readout-row">
    <span class="readout" class:is-idle={hasNoMeasure} aria-hidden="true">{readout}</span>
    <span class="stats">
      {#if sizeLabel}
        <span class="stat">{sizeLabel}</span>
      {/if}
      {#if showLiveStats && entry.speedBytesPerSec > 0}
        <span class="stat">{formatSpeed(entry.speedBytesPerSec)}</span>
      {/if}
      {#if showLiveStats && entry.etaSeconds}
        <span class="stat">Reste {formatDuration(entry.etaSeconds)}</span>
      {/if}
    </span>
  </div>

  <div
    class="meter"
    role="progressbar"
    aria-label="Progression du téléchargement"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={isIndeterminate ? undefined : Math.round(percentage)}
    aria-valuetext={valueText}
  >
    <div
      class="meter-fill"
      class:is-indeterminate={isIndeterminate}
      style:--progress={isIndeterminate ? null : percentage}
    ></div>
  </div>

  {#if entry.status === 'completed' && entry.filePath}
    <p class="result-line">
      Enregistré dans{' '}
      <button
        type="button"
        class="file-path-btn"
        onclick={handleReveal}
        title="Cliquer pour afficher dans le dossier"
      >
        {entry.filePath}
      </button>
    </p>
  {:else if entry.status === 'failed' && entry.errorMessage}
    <p class="result-line error-line" role="alert">{entry.errorMessage}</p>
  {/if}
</article>

<style>
  .download-card {
    --meter-color: var(--color-accent);
    --status-color: var(--color-accent-strong);

    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-5) var(--space-5);
    background-color: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-panel);
    animation: rise var(--duration-base) var(--ease-out);
  }

  .download-card.is-completed {
    --meter-color: var(--color-success-meter);
    --status-color: var(--color-success);
  }

  .download-card.is-failed {
    --meter-color: var(--color-danger);
    --status-color: var(--color-danger);
  }

  .download-card.is-on-hold {
    --meter-color: var(--color-border-strong);
    --status-color: var(--color-text-muted);
  }

  .download-card.is-cancelled {
    --meter-color: var(--color-border-strong);
    --status-color: var(--color-text-muted);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
  }

  .status-line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    color: var(--status-color);
  }

  .status-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  /* Voyant « REC » pendant la capture */
  .rec-dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    background-color: var(--color-accent);
    animation: blink 1.4s ease-in-out infinite;
  }

  /* Voyant éteint : en attente ou en pause */
  .hold-dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    border: 2px solid currentColor;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    flex-shrink: 0;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
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

  .icon-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .status-label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .download-title {
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Compteur et mesures */
  .readout-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-4);
    margin-top: var(--space-1);
  }

  .readout {
    font-family: var(--font-mono);
    font-size: var(--text-2xl);
    font-weight: 500;
    line-height: 1;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
    color: var(--color-text);
  }

  .readout.is-idle {
    color: var(--color-text-subtle);
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    row-gap: var(--space-1);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
    color: var(--color-text-muted);
  }

  .stat + .stat::before {
    content: '·';
    margin: 0 var(--space-2);
    color: var(--color-text-subtle);
  }

  /*
   * Indicateur à segments, façon VU-mètre : un masque découpe la piste et le remplissage en
   * segments de 6 px séparés de 2 px. La largeur remplie est arrondie au segment entier
   * (`round()`), avec repli sur une largeur continue pour les moteurs qui ne le gèrent pas.
   */
  .meter {
    --segment: 6px;
    --segment-gap: 2px;
    --segment-step: calc(var(--segment) + var(--segment-gap));

    position: relative;
    height: 14px;
    margin-top: var(--space-1);
    overflow: hidden;
    background-color: oklch(0.9 0.006 var(--neutral-hue));
    -webkit-mask-image: repeating-linear-gradient(
      to right,
      black 0 var(--segment),
      transparent var(--segment) var(--segment-step)
    );
    mask-image: repeating-linear-gradient(
      to right,
      black 0 var(--segment),
      transparent var(--segment) var(--segment-step)
    );
  }

  .meter-fill {
    height: 100%;
    width: calc(var(--progress, 0) * 1%);
    width: round(down, calc(var(--progress, 0) * 1%), var(--segment-step));
    background-color: var(--meter-color);
    transition:
      width var(--duration-base) var(--ease-out),
      background-color var(--duration-base) var(--ease-out);
  }

  /* Sans mesure possible : un groupe de segments balaie la piste */
  .meter-fill.is-indeterminate {
    width: 20%;
    animation: scan 1.6s ease-in-out infinite alternate;
  }

  .card-btn {
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background-color: var(--color-surface-sunken);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
    flex-shrink: 0;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out),
      border-color var(--duration-fast) var(--ease-out);
  }

  .cancel-btn:hover:not(:disabled) {
    border-color: var(--color-danger-border);
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
  }

  .dismiss-btn:hover {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .open-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    border-color: var(--color-accent-border);
    background-color: var(--color-accent-subtle, oklch(0.96 0.02 var(--accent-hue)));
    color: var(--color-accent-strong);
  }

  .open-btn:hover {
    background-color: var(--color-accent);
    border-color: var(--color-accent);
    color: #ffffff;
  }

  .retry-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    border-color: var(--color-border-strong);
    background-color: var(--color-surface);
    color: var(--color-text);
  }

  .retry-btn:hover {
    background-color: var(--color-surface-hover);
    border-color: var(--color-accent-strong);
    color: var(--color-accent-strong);
  }

  .card-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .result-line {
    margin-top: var(--space-2);
    font-size: var(--text-sm);
    line-height: 1.45;
    color: var(--color-text-muted);
  }

  .file-path-btn {
    display: inline;
    padding: 0;
    border: none;
    background: none;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-text);
    word-break: break-all;
    text-align: left;
    text-decoration: underline;
    text-decoration-color: var(--color-border-strong);
    text-underline-offset: 2px;
    cursor: pointer;
    transition:
      color var(--duration-fast) var(--ease-out),
      text-decoration-color var(--duration-fast) var(--ease-out);
  }

  .file-path-btn:hover {
    color: var(--color-accent-strong);
    text-decoration-color: var(--color-accent-strong);
  }

  .error-line {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-danger-border);
    border-radius: var(--radius-md);
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
  }

  @keyframes scan {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(400%);
    }
  }

  @keyframes blink {
    50% {
      opacity: 0.25;
    }
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .rec-dot {
      animation: none;
    }

    /* Sans mouvement, une piste pleine atténuée signale l'activité. */
    .meter-fill.is-indeterminate {
      width: 100%;
      opacity: 0.35;
      animation: none;
      transform: none;
    }
  }
</style>
