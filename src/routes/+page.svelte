<script lang="ts">
  import DestinationPicker from '$lib/components/DestinationPicker.svelte';
  import DownloadHistory from '$lib/components/DownloadHistory.svelte';
  import DownloadProgress from '$lib/components/DownloadProgress.svelte';
  import QualitySelector from '$lib/components/QualitySelector.svelte';
  import UrlInput from '$lib/components/UrlInput.svelte';
  import { onMount, untrack } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { destination } from '$lib/stores/destination.svelte';
  import { createDownloadNotifier } from '$lib/stores/download-notifications';
  import {
    downloads,
    isTerminalStatus,
    MAX_CONCURRENT_DOWNLOADS
  } from '$lib/stores/downloads.svelte';
  import { history, type HistoryEntry } from '$lib/stores/history.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import type { CommandError, VideoMetadata } from '$lib/types/download';
  import { formatDuration, formatErrorMessage } from '$lib/utils/format';

  let inspectedVideo = $state<VideoMetadata | null>(null);
  // Qualité choisie, conservée d'une vidéo à l'autre quand elle reste disponible.
  let selectedQuality = $state('best');
  let currentError = $state<CommandError | null>(null);
  let isInspecting = $state(false);

  let isAdding = $state(false);
  let downloadStartError = $state<string | null>(null);

  let finishedCount = $derived(
    downloads.ordered.filter((entry) => isTerminalStatus(entry.status)).length
  );

  // Notifications du cycle de vie de chaque téléchargement de la file
  const notifyDownload = createDownloadNotifier();

  interface ExtensionDownloadPayload {
    url: string;
    title?: string | null;
    format_id?: string | null;
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    void listen<ExtensionDownloadPayload>('extension-download', (event) => {
      const payload = event.payload;
      const title = payload.title?.trim() || 'Vidéo web';
      const formatId = payload.format_id?.trim() || 'best';

      void addDownload({
        url: payload.url,
        title,
        formatId,
        outputDir: destination.selected ?? undefined
      }).then((error) => {
        if (error) {
          toasts.error({
            title: 'Échec de capture extension',
            message: error
          });
        } else {
          toasts.success({
            title: 'Capture vidéo détectée',
            message: `${title} a été ajouté à la file d’attente.`
          });
        }
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  });

  $effect(() => {
    for (const entry of downloads.ordered) {
      // Champs dont l'évolution peut produire une notification : seuls ceux-ci relancent l'effet.
      void [entry.status, entry.progressPercentage, entry.filePath, entry.errorMessage];
      // Le notificateur lit et modifie la liste des toasts : hors suivi pour ne pas boucler.
      untrack(() => notifyDownload(entry));
    }
  });

  /**
   * Ajoute un téléchargement à la file (nouveau ou relancé depuis l'historique).
   * Renvoie le message d'erreur si l'ajout lui-même échoue.
   */
  async function addDownload(params: {
    url: string;
    title: string;
    formatId: string;
    outputDir?: string;
  }): Promise<string | null> {
    isAdding = true;
    try {
      await downloads.enqueue(params.title, {
        url: params.url,
        formatId: params.formatId,
        outputDir: params.outputDir,
        fileName: params.title
      });
      if (params.outputDir) {
        destination.remember(params.outputDir);
      }
      return null;
    } catch (err: unknown) {
      return formatErrorMessage(err);
    } finally {
      isAdding = false;
    }
  }

  async function handleDownload() {
    if (!inspectedVideo || isAdding) return;
    downloadStartError = null;
    downloadStartError = await addDownload({
      url: inspectedVideo.source_url,
      title: inspectedVideo.title,
      formatId: selectedQuality,
      outputDir: destination.selected ?? undefined
    });
  }

  // Relance avec les mêmes réglages qu'à l'origine (qualité, dossier) ; le backend revérifie tout.
  async function handleRetry(entry: HistoryEntry) {
    const error = await addDownload({
      url: entry.sourceUrl,
      title: entry.title,
      formatId: entry.formatId,
      outputDir: entry.outputDir
    });
    if (error) {
      toasts.error({ title: 'Relance impossible', message: error });
    }
  }

  // Lampe d'état de l'en-tête : vue d'ensemble de la file, lisible d'un coup d'œil.
  let lampState = $derived<'idle' | 'active'>(downloads.runningCount > 0 ? 'active' : 'idle');

  let deviceStatus = $derived.by(() => {
    const running = downloads.runningCount;
    if (running > 1) return `${running} captures en cours`;
    if (running === 1) return 'Capture en cours';
    return downloads.waitingCount > 0 ? 'En attente' : 'Prêt';
  });

  let queueSummary = $derived.by(() => {
    const parts: string[] = [];
    if (downloads.runningCount > 0) parts.push(`${downloads.runningCount} en cours`);
    if (downloads.waitingCount > 0) parts.push(`${downloads.waitingCount} en attente`);
    const paused = downloads.ordered.filter((e) => e.status === 'paused').length;
    if (paused > 0) parts.push(`${paused} en pause`);
    if (finishedCount > 0) parts.push(`${finishedCount} terminé${finishedCount > 1 ? 's' : ''}`);
    return parts.join(' · ');
  });

  let errorAdvice = $derived.by(() => {
    if (!currentError) return null;
    switch (currentError.kind) {
      case 'InvalidUrl':
        return 'Vérifiez que le lien commence bien par http:// ou https:// et qu’il ne comporte pas de faute de frappe.';
      case 'UnsupportedPlatform':
        return 'Cette plateforme n’est pas reconnue. Pour les sites web généraux, essayez de coller le lien direct vers le fichier .mp4 ou le flux .m3u8.';
      case 'VideoNotFound':
        return 'La vidéo semble indisponible, supprimée, privée ou soumise à une restriction d’accès.';
      case 'NetworkTimeout':
      case 'NetworkError':
        return 'Le réseau n’a pas répondu à temps. Vérifiez votre connexion Internet et réessayez.';
      case 'ExtractionFailed':
      default:
        return 'yt-dlp ou l’extracteur n’a pas pu récupérer les flux média pour cette adresse.';
    }
  });

  function handleInspect(metadata: VideoMetadata) {
    isInspecting = false;
    inspectedVideo = metadata;
    const available = metadata.available_formats.map((f) => f.format_id);
    if (!available.includes(selectedQuality)) {
      selectedQuality = available[0] ?? 'best';
    }
    currentError = null;
  }

  function handleError(error: CommandError | null) {
    isInspecting = false;
    currentError = error;
    if (error) {
      inspectedVideo = null;
    }
  }

  function handleClear() {
    isInspecting = false;
    inspectedVideo = null;
    currentError = null;
  }
</script>

<div class="app-shell">
  <header class="app-header">
    <div class="brand">
      <img src="/favicon.png" alt="ktk-download" class="brand-logo" width="24" height="24" />
      <span class="lamp" data-state={lampState} aria-hidden="true"></span>
      <h1 class="wordmark">ktk-download</h1>
    </div>
    <p class="device-status" data-state={lampState}>{deviceStatus}</p>
  </header>

  <main class="app-main">
    <section class="section-input" aria-label="Lien de la vidéo">
      <UrlInput
        onInspect={handleInspect}
        onError={handleError}
        onLoading={(loading) => {
          isInspecting = loading;
          if (loading) {
            currentError = null;
          }
        }}
        onClear={handleClear}
        autofocus={true}
      />
    </section>

    {#if isInspecting}
      <section
        class="panel video-card video-card-skeleton"
        aria-label="Analyse de la vidéo en cours"
        aria-busy="true"
      >
        <div class="video-main">
          <div class="thumbnail skeleton-thumb">
            <svg
              class="skeleton-play-icon"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              aria-hidden="true"
            >
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </div>

          <div class="video-details">
            <span class="skeleton-pill"></span>
            <div class="skeleton-line skeleton-title-1"></div>
            <div class="skeleton-line skeleton-title-2"></div>
            <div class="skeleton-line skeleton-author"></div>
          </div>
        </div>

        <div class="skeleton-footer">
          <span class="skeleton-spinner" aria-hidden="true"></span>
          <span class="skeleton-label"
            >Analyse du flux vidéo et résolution des qualités disponibles…</span
          >
        </div>
      </section>
    {:else if inspectedVideo}
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
            />
          {/if}

          <div class="output-row">
            <DestinationPicker />

            <button type="button" class="download-btn" onclick={handleDownload} disabled={isAdding}>
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
              <!-- Emplacements tous occupés : le téléchargement attendra son tour dans la file -->
              <span>
                {downloads.runningCount >= MAX_CONCURRENT_DOWNLOADS
                  ? 'Ajouter à la file'
                  : 'Télécharger'}
              </span>
            </button>
          </div>
        </div>

        {#if downloadStartError}
          <p class="download-start-error" role="alert">{downloadStartError}</p>
        {/if}
      </section>
    {:else if currentError}
      <section class="panel error-card" role="alert" aria-live="assertive">
        <div class="error-header">
          <div class="error-icon-box" aria-hidden="true">
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
          </div>
          <div class="error-text-group">
            <h2 class="error-title">Échec de l'analyse vidéo</h2>
            <p class="error-message">{currentError.message}</p>
          </div>
          <button
            type="button"
            class="error-dismiss-btn"
            onclick={() => (currentError = null)}
            aria-label="Fermer ce message d'erreur"
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

        {#if errorAdvice}
          <div class="error-advice">
            <span class="advice-badge">Conseil</span>
            <span class="advice-text">{errorAdvice}</span>
          </div>
        {/if}
      </section>
    {:else if downloads.ordered.length === 0 && history.entries.length === 0}
      <section class="panel sources" aria-labelledby="sources-heading">
        <div class="sources-header">
          <h2 id="sources-heading" class="sources-heading">Prêt à télécharger</h2>
          <span class="sources-badge">Multi-sources</span>
        </div>
        <p class="sources-lead">
          Collez le lien d'une vidéo dans le champ ci-dessus pour inspecter ses formats et lancer la
          capture haute vitesse.
        </p>

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
            <dd>Vidéos standard, Shorts, listes et conversion audio MP3</dd>
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
              Flux directs & Web
            </dt>
            <dd>
              Flux streaming HLS <code>.m3u8</code> et fichiers vidéo <code>.mp4</code>,
              <code>.webm</code>, <code>.mkv</code>
            </dd>
          </div>
        </dl>

        <div class="sources-tips">
          <span class="tip-kbd">Entrée</span> pour analyser
          <span class="tip-sep">·</span>
          <span class="tip-kbd">Échap</span> pour effacer
          <span class="tip-sep">·</span>
          <span class="tip-kbd">Ctrl+V</span> pour coller
        </div>
      </section>
    {/if}

    {#if downloads.ordered.length > 0}
      <section class="queue" aria-labelledby="queue-heading">
        <header class="queue-header">
          <h2 id="queue-heading" class="queue-heading">
            File d'attente <span class="queue-summary">{queueSummary}</span>
          </h2>
          {#if finishedCount > 0}
            <button
              type="button"
              class="queue-clear-btn"
              onclick={() => downloads.dismissFinished()}
            >
              Retirer les terminés
            </button>
          {/if}
        </header>

        <ol class="queue-list">
          {#each downloads.ordered as entry, index (entry.id)}
            <li>
              <DownloadProgress
                {entry}
                onCancel={() => downloads.cancel(entry.id)}
                onDismiss={() => downloads.dismiss(entry.id)}
                onPause={() => downloads.pause(entry.id)}
                onResume={() => downloads.resume(entry.id)}
                onRetry={() => {
                  downloads.dismiss(entry.id);
                  addDownload({
                    url: entry.params.url,
                    title: entry.title,
                    formatId: entry.params.formatId ?? 'best',
                    outputDir: entry.params.outputDir
                  });
                }}
                onMoveUp={index > 0 ? () => downloads.move(entry.id, -1) : undefined}
                onMoveDown={index < downloads.ordered.length - 1
                  ? () => downloads.move(entry.id, 1)
                  : undefined}
              />
            </li>
          {/each}
        </ol>
      </section>
    {/if}

    {#if history.entries.length > 0}
      <DownloadHistory onRetry={handleRetry} />
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

  .brand-logo {
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm, 6px);
    flex-shrink: 0;
    box-shadow: 0 1px 2px oklch(0.2 0.01 var(--neutral-hue) / 0.1);
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

  /* File d'attente */
  .queue {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .queue-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: 0 var(--space-1);
  }

  .queue-heading {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-2);
    font-size: var(--text-sm);
    font-weight: 650;
    color: var(--color-text);
  }

  .queue-summary {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    color: var(--color-text-muted);
  }

  .queue-clear-btn {
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

  .queue-clear-btn:hover {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .queue-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    list-style: none;
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

  .sources-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-1);
  }

  .sources-heading {
    font-size: var(--text-base);
    font-weight: 700;
    color: var(--color-text);
  }

  .sources-badge {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 2px var(--space-2);
    border-radius: var(--radius-full);
    background-color: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    color: var(--color-text-muted);
  }

  .sources-lead {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
    margin-bottom: var(--space-4);
    line-height: 1.45;
  }

  .sources-tips {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--color-border);
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }

  .tip-kbd {
    display: inline-flex;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background-color: var(--color-surface-sunken);
    border: 1px solid var(--color-border-strong);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 600;
    color: var(--color-text);
  }

  .tip-sep {
    color: var(--color-text-subtle);
  }

  /* Carte Squelette de chargement */
  .video-card-skeleton {
    overflow: hidden;
  }

  .skeleton-thumb {
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(
      90deg,
      var(--color-surface-sunken) 25%,
      var(--color-surface-hover) 50%,
      var(--color-surface-sunken) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.8s infinite;
    color: var(--color-text-subtle);
  }

  .skeleton-play-icon {
    opacity: 0.35;
    animation: lamp-pulse 1.4s ease-in-out infinite;
  }

  .skeleton-pill {
    width: 80px;
    height: 14px;
    border-radius: var(--radius-full);
    background: linear-gradient(
      90deg,
      var(--color-surface-sunken) 25%,
      var(--color-surface-hover) 50%,
      var(--color-surface-sunken) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.8s infinite;
  }

  .skeleton-line {
    border-radius: var(--radius-sm);
    background: linear-gradient(
      90deg,
      var(--color-surface-sunken) 25%,
      var(--color-surface-hover) 50%,
      var(--color-surface-sunken) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.8s infinite;
  }

  .skeleton-title-1 {
    width: 90%;
    height: 18px;
    margin-top: var(--space-1);
  }

  .skeleton-title-2 {
    width: 60%;
    height: 18px;
  }

  .skeleton-author {
    width: 35%;
    height: 13px;
    margin-top: var(--space-1);
  }

  .skeleton-footer {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background-color: var(--color-surface-sunken);
    border-top: 1px solid var(--color-border);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  }

  .skeleton-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--color-border-strong);
    border-top-color: var(--color-accent-strong);
    border-radius: var(--radius-full);
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  .skeleton-label {
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }

  /* Carte d'erreur dédiée (Error Boundary) */
  .error-card {
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--color-danger-border);
    background-color: var(--color-surface);
    box-shadow: var(--shadow-panel);
    border-radius: var(--radius-lg);
    animation: rise var(--duration-base) var(--ease-out);
  }

  .error-header {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .error-icon-box {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-md);
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
    flex-shrink: 0;
  }

  .error-text-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .error-title {
    font-size: var(--text-sm);
    font-weight: 650;
    color: var(--color-danger);
  }

  .error-message {
    font-size: var(--text-sm);
    color: var(--color-text);
    line-height: 1.45;
    word-break: break-word;
  }

  .error-dismiss-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .error-dismiss-btn:hover {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .error-advice {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin-top: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    background-color: var(--color-surface-sunken);
    font-size: var(--text-xs);
    line-height: 1.45;
    color: var(--color-text-muted);
  }

  .advice-badge {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background-color: var(--color-border);
    color: var(--color-text);
    flex-shrink: 0;
  }

  .advice-text {
    flex: 1;
  }

  @keyframes shimmer {
    0% {
      background-position: -200% 0;
    }
    100% {
      background-position: 200% 0;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
