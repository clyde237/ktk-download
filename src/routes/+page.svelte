<script lang="ts">
  import DestinationPicker from '$lib/components/DestinationPicker.svelte';
  import DownloadHistory from '$lib/components/DownloadHistory.svelte';
  import DownloadProgress from '$lib/components/DownloadProgress.svelte';
  import QualitySelector from '$lib/components/QualitySelector.svelte';
  import UrlInput from '$lib/components/UrlInput.svelte';
  import { untrack } from 'svelte';
  import { destination } from '$lib/stores/destination.svelte';
  import { createDownloadNotifier } from '$lib/stores/download-notifications';
  import { downloads, isTerminalStatus } from '$lib/stores/downloads.svelte';
  import { history, type HistoryEntry } from '$lib/stores/history.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import type { CommandError, VideoMetadata } from '$lib/types/download';
  import { formatDuration, formatErrorMessage } from '$lib/utils/format';

  let inspectedVideo = $state<VideoMetadata | null>(null);
  // Qualité choisie, conservée d'une vidéo à l'autre quand elle reste disponible.
  let selectedQuality = $state('best');
  let currentError = $state<CommandError | null>(null);

  // Un seul téléchargement à la fois tant que la file d'attente (phase 4) n'existe pas.
  let activeDownloadId = $state<string | null>(null);
  let isStartingDownload = $state(false);
  let downloadStartError = $state<string | null>(null);

  let activeDownload = $derived(
    activeDownloadId ? (downloads.entries[activeDownloadId] ?? null) : null
  );
  let isDownloading = $derived(
    isStartingDownload || (activeDownload !== null && !isTerminalStatus(activeDownload.status))
  );

  // Notifications du cycle de vie du téléchargement actif
  const notifyDownload = createDownloadNotifier();

  $effect(() => {
    const entry = activeDownload;
    if (!entry) return;
    // Champs dont l'évolution peut produire une notification : seuls ceux-ci relancent l'effet.
    void [entry.status, entry.progressPercentage, entry.filePath, entry.errorMessage];
    // Le notificateur lit et modifie la liste des toasts : hors suivi pour ne pas boucler.
    untrack(() => notifyDownload(entry));
  });

  /**
   * Lance un téléchargement (nouveau ou relancé depuis l'historique).
   * Renvoie le message d'erreur si le lancement lui-même échoue.
   */
  async function startDownload(params: {
    url: string;
    title: string;
    formatId: string;
    outputDir?: string;
  }): Promise<string | null> {
    if (isDownloading) return null;

    isStartingDownload = true;
    if (activeDownloadId) {
      downloads.dismiss(activeDownloadId);
    }

    try {
      activeDownloadId = await downloads.start(params.url, params.title, {
        outputDir: params.outputDir,
        formatId: params.formatId,
        fileName: params.title
      });
      if (params.outputDir) {
        destination.remember(params.outputDir);
      }
      return null;
    } catch (err: unknown) {
      activeDownloadId = null;
      return formatErrorMessage(err);
    } finally {
      isStartingDownload = false;
    }
  }

  async function handleDownload() {
    if (!inspectedVideo) return;
    downloadStartError = null;
    downloadStartError = await startDownload({
      url: inspectedVideo.source_url,
      title: inspectedVideo.title,
      formatId: selectedQuality,
      outputDir: destination.selected ?? undefined
    });
  }

  // Relance avec les mêmes réglages qu'à l'origine (qualité, dossier) ; le backend revérifie tout.
  async function handleRetry(entry: HistoryEntry) {
    const error = await startDownload({
      url: entry.sourceUrl,
      title: entry.title,
      formatId: entry.formatId,
      outputDir: entry.outputDir
    });
    if (error) {
      toasts.error({ title: 'Relance impossible', message: error });
    }
  }

  function handleCancelDownload() {
    if (activeDownloadId) {
      downloads.cancel(activeDownloadId);
    }
  }

  function handleDismissDownload() {
    if (!activeDownloadId) return;
    downloads.dismiss(activeDownloadId);
    activeDownloadId = null;
  }

  // Lampe d'état de l'en-tête : vue d'ensemble de la capture, lisible d'un coup d'œil.
  type LampState = 'idle' | 'active' | 'completed' | 'failed';
  const LAMP_LABELS: Record<LampState, string> = {
    idle: 'Prêt',
    active: 'Capture en cours',
    completed: 'Terminé',
    failed: 'Échec'
  };

  let lampState = $derived.by((): LampState => {
    if (!activeDownload) return isStartingDownload ? 'active' : 'idle';
    switch (activeDownload.status) {
      case 'completed':
        return 'completed';
      case 'failed':
        return 'failed';
      case 'cancelled':
        return 'idle';
      default:
        return 'active';
    }
  });

  function handleInspect(metadata: VideoMetadata) {
    inspectedVideo = metadata;
    const available = metadata.available_formats.map((f) => f.format_id);
    if (!available.includes(selectedQuality)) {
      selectedQuality = available[0] ?? 'best';
    }
    currentError = null;
  }

  function handleError(error: CommandError | null) {
    currentError = error;
    if (error) {
      inspectedVideo = null;
    }
  }

  function handleClear() {
    inspectedVideo = null;
    currentError = null;
  }
</script>

<div class="app-shell">
  <header class="app-header">
    <div class="brand">
      <span class="lamp" data-state={lampState} aria-hidden="true"></span>
      <h1 class="wordmark">ktk-download</h1>
    </div>
    <p class="device-status" data-state={lampState}>{LAMP_LABELS[lampState]}</p>
  </header>

  <main class="app-main">
    <section class="section-input" aria-label="Lien de la vidéo">
      <UrlInput
        onInspect={handleInspect}
        onError={handleError}
        onClear={handleClear}
        disabled={isDownloading}
        autofocus={true}
      />
    </section>

    {#if inspectedVideo}
      <section class="panel video-card" aria-label="Vidéo analysée">
        <div class="video-main">
          {#if inspectedVideo.thumbnail_url}
            <div class="thumbnail">
              <img src={inspectedVideo.thumbnail_url} alt="" loading="lazy" />
              {#if inspectedVideo.duration_seconds}
                <span class="duration">{formatDuration(inspectedVideo.duration_seconds)}</span>
              {/if}
            </div>
          {/if}

          <div class="video-details">
            <p class="source-tag">{inspectedVideo.source_platform}</p>

            <h2 class="video-title" title={inspectedVideo.title}>{inspectedVideo.title}</h2>

            {#if inspectedVideo.author}
              <p class="video-author">{inspectedVideo.author}</p>
            {/if}
          </div>
        </div>

        <!-- Réglages de sortie : qualité, dossier, puis lancement de la capture -->
        <div class="video-footer">
          {#if inspectedVideo.available_formats.length > 1}
            <QualitySelector
              options={inspectedVideo.available_formats}
              value={selectedQuality}
              onChange={(formatId) => (selectedQuality = formatId)}
              disabled={isDownloading}
            />
          {/if}

          <div class="output-row">
            <DestinationPicker disabled={isDownloading} />

            <button
              type="button"
              class="download-btn"
              onclick={handleDownload}
              disabled={isDownloading}
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              <span>{isDownloading ? 'Téléchargement…' : 'Télécharger'}</span>
            </button>
          </div>
        </div>

        {#if downloadStartError}
          <p class="download-start-error" role="alert">{downloadStartError}</p>
        {/if}
      </section>
    {:else if !currentError && !activeDownload && history.entries.length === 0}
      <section class="panel sources" aria-labelledby="sources-heading">
        <h2 id="sources-heading" class="sources-heading">Sources prises en charge</h2>
        <dl class="sources-list">
          <div class="source-row">
            <dt>
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="currentColor"
                aria-hidden="true"
              >
                <path
                  d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"
                />
              </svg>
              YouTube
            </dt>
            <dd>Vidéos, Shorts et audio MP3</dd>
          </div>
          <div class="source-row">
            <dt>
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                aria-hidden="true"
              >
                <circle cx="12" cy="12" r="10" />
                <polygon points="10 8 16 12 10 16 10 8" />
              </svg>
              Liens directs
            </dt>
            <dd>Fichiers <code>.mp4</code> et flux HLS <code>.m3u8</code></dd>
          </div>
        </dl>
        <p class="sources-hint">Collez un lien ci-dessus, puis lancez l'analyse.</p>
      </section>
    {/if}

    {#if activeDownload}
      <section class="section-download" aria-label="Téléchargement">
        <DownloadProgress
          entry={activeDownload}
          onCancel={handleCancelDownload}
          onDismiss={handleDismissDownload}
        />
      </section>
    {/if}

    {#if history.entries.length > 0}
      <DownloadHistory onRetry={handleRetry} retryDisabled={isDownloading} />
    {/if}
  </main>
</div>

<style>
  .app-shell {
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* En-tête : nom de l'appareil et lampe d'état */
  .app-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-6);
    background-color: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }

  .wordmark {
    font-size: var(--text-xl);
    font-weight: 800;
    font-stretch: 125%;
    letter-spacing: -0.02em;
    line-height: 1;
    color: var(--color-text);
  }

  .lamp {
    width: 10px;
    height: 10px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
    background-color: var(--color-border-strong);
    box-shadow: inset 0 0 0 1px oklch(0.22 0.012 var(--neutral-hue) / 0.15);
    transition: background-color var(--duration-base) var(--ease-out);
  }

  .lamp[data-state='active'] {
    background-color: var(--color-accent);
    box-shadow: 0 0 0 3px oklch(0.64 0.19 var(--signal-hue) / 0.22);
    animation: lamp-pulse 1.4s ease-in-out infinite;
  }

  .lamp[data-state='completed'] {
    background-color: var(--color-success-meter);
  }

  .lamp[data-state='failed'] {
    background-color: var(--color-danger);
  }

  .device-status {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .device-status[data-state='active'] {
    color: var(--color-accent-strong);
  }

  .device-status[data-state='completed'] {
    color: var(--color-success);
  }

  .device-status[data-state='failed'] {
    color: var(--color-danger);
  }

  .app-main {
    width: 100%;
    max-width: 760px;
    margin: 0 auto;
    /* Marge basse : le bas de page défile au-dessus d'un toast affiché en bas à droite. */
    padding: var(--space-8) var(--space-6) calc(var(--space-12) + 72px);
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  .panel {
    background-color: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-panel);
    animation: rise var(--duration-base) var(--ease-out);
  }

  /* Vidéo analysée */
  .video-card {
    display: flex;
    flex-direction: column;
  }

  .video-main {
    display: flex;
    gap: var(--space-5);
    padding: var(--space-4);
  }

  .video-footer {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-4) var(--space-4);
    border-top: 1px solid var(--color-border);
    background-color: var(--color-surface-sunken);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  }

  .thumbnail {
    position: relative;
    width: 208px;
    aspect-ratio: 16 / 9;
    border-radius: var(--radius-md);
    overflow: hidden;
    background-color: var(--color-surface-sunken);
    flex-shrink: 0;
  }

  .thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .duration {
    position: absolute;
    right: var(--space-2);
    bottom: var(--space-2);
    padding: 1px var(--space-1);
    border-radius: var(--radius-sm);
    background-color: oklch(0.22 0.012 var(--neutral-hue) / 0.85);
    color: var(--color-on-accent);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-variant-numeric: tabular-nums;
  }

  .video-details {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
    flex: 1;
  }

  .source-tag {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
  }

  .video-title {
    font-size: var(--text-lg);
    font-weight: 650;
    line-height: 1.3;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .video-author {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
  }

  .output-row {
    display: flex;
    align-items: flex-end;
    gap: var(--space-3);
  }

  .download-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    background-color: var(--color-accent-strong);
    color: var(--color-on-accent);
    font-size: var(--text-sm);
    font-weight: 650;
    box-shadow: var(--shadow-control);
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .download-btn:hover:not(:disabled) {
    background-color: var(--color-accent-stronger);
  }

  .download-btn:disabled {
    background-color: var(--color-surface-hover);
    color: var(--color-text-muted);
    box-shadow: none;
    cursor: not-allowed;
  }

  .download-start-error {
    padding: 0 var(--space-4) var(--space-4);
    background-color: var(--color-surface-sunken);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
    font-size: var(--text-sm);
    color: var(--color-danger);
  }

  /* État vide : sources prises en charge */
  .sources {
    padding: var(--space-5) var(--space-5) var(--space-4);
  }

  .sources-heading {
    font-size: var(--text-sm);
    font-weight: 650;
    color: var(--color-text);
    margin-bottom: var(--space-2);
  }

  .sources-list {
    display: flex;
    flex-direction: column;
  }

  .source-row {
    display: grid;
    grid-template-columns: 11rem 1fr;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) 0;
    border-top: 1px solid var(--color-border);
  }

  .source-row dt {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 600;
    color: var(--color-text);
  }

  .source-row dt svg {
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .source-row dd {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
  }

  .source-row code {
    font-family: var(--font-mono);
    font-size: 0.92em;
    color: var(--color-text);
  }

  .sources-hint {
    margin-top: var(--space-2);
    padding-top: var(--space-3);
    border-top: 1px solid var(--color-border);
    font-size: var(--text-sm);
    color: var(--color-text-muted);
  }

  @keyframes lamp-pulse {
    50% {
      box-shadow: 0 0 0 5px oklch(0.64 0.19 var(--signal-hue) / 0.08);
    }
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @media (max-width: 640px) {
    .app-header {
      padding: var(--space-3) var(--space-4);
    }

    .app-main {
      padding: var(--space-6) var(--space-4) var(--space-10);
    }

    .video-main {
      flex-direction: column;
    }

    .output-row {
      flex-direction: column;
      align-items: stretch;
    }

    .thumbnail {
      width: 100%;
    }

    .source-row {
      grid-template-columns: 1fr;
      gap: var(--space-1);
    }
  }
</style>
