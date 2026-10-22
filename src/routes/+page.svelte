<script lang="ts">
  import UrlInput from '$lib/components/UrlInput.svelte';
  import type { CommandError, VideoMetadata } from '$lib/types/download';
  import { formatDuration } from '$lib/utils/format';

  let inspectedVideo = $state<VideoMetadata | null>(null);
  let currentError = $state<CommandError | null>(null);

  function handleInspect(metadata: VideoMetadata) {
    inspectedVideo = metadata;
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

<main class="app-layout">
  <!-- En-tête de l'application -->
  <header class="app-header">
    <div class="brand">
      <div class="brand-logo" aria-hidden="true">
        <svg
          width="26"
          height="26"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
      </div>
      <div>
        <h1 class="brand-title">ktk-download</h1>
        <p class="brand-subtitle">Téléchargeur universel de vidéos haute performance</p>
      </div>
    </div>
  </header>

  <!-- Zone de saisie et de validation d'URL -->
  <section class="section-input">
    <UrlInput
      onInspect={handleInspect}
      onError={handleError}
      onClear={handleClear}
      autofocus={true}
    />
  </section>

  <!-- Résultat d'inspection vidéo -->
  {#if inspectedVideo}
    <section class="section-preview" aria-label="Aperçu de la vidéo analysée">
      <div class="video-card">
        {#if inspectedVideo.thumbnail_url}
          <div class="thumbnail-wrapper">
            <img
              src={inspectedVideo.thumbnail_url}
              alt={inspectedVideo.title}
              class="thumbnail-img"
              loading="lazy"
            />
            {#if inspectedVideo.duration_seconds}
              <span class="duration-badge">
                {formatDuration(inspectedVideo.duration_seconds)}
              </span>
            {/if}
          </div>
        {/if}

        <div class="video-details">
          <div class="source-tag">
            <span>{inspectedVideo.source_platform}</span>
          </div>

          <h2 class="video-title" title={inspectedVideo.title}>
            {inspectedVideo.title}
          </h2>

          {#if inspectedVideo.author}
            <p class="video-author">
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
              <span>{inspectedVideo.author}</span>
            </p>
          {/if}

          {#if inspectedVideo.available_formats && inspectedVideo.available_formats.length > 0}
            <div class="formats-summary">
              <span class="formats-count">
                {inspectedVideo.available_formats.length} format{inspectedVideo.available_formats
                  .length > 1
                  ? 's'
                  : ''} disponible{inspectedVideo.available_formats.length > 1 ? 's' : ''}
              </span>
            </div>
          {/if}
        </div>
      </div>
    </section>
  {:else if !currentError}
    <!-- Guide visuel des plateformes supportées -->
    <section class="section-supported">
      <h3 class="supported-heading">Plateformes compatibles</h3>
      <div class="supported-grid">
        <div class="platform-card card-youtube">
          <div class="platform-icon" aria-hidden="true">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
              <path
                d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"
              />
            </svg>
          </div>
          <div class="platform-info">
            <div class="platform-name">YouTube</div>
            <div class="platform-desc">Vidéos standard, Shorts, listes et audio MP3</div>
          </div>
        </div>

        <div class="platform-card card-movibox">
          <div class="platform-icon" aria-hidden="true">
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" />
              <line x1="7" y1="2" x2="7" y2="22" />
              <line x1="17" y1="2" x2="17" y2="22" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <line x1="2" y1="7" x2="7" y2="7" />
              <line x1="2" y1="17" x2="7" y2="17" />
              <line x1="17" y1="17" x2="22" y2="17" />
              <line x1="17" y1="7" x2="22" y2="7" />
            </svg>
          </div>
          <div class="platform-info">
            <div class="platform-name">MoviBox</div>
            <div class="platform-desc">Films et séries hébergés sur movibox.net</div>
          </div>
        </div>

        <div class="platform-card card-generic">
          <div class="platform-icon" aria-hidden="true">
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <circle cx="12" cy="12" r="10" />
              <polygon points="10 8 16 12 10 16 10 8" />
            </svg>
          </div>
          <div class="platform-info">
            <div class="platform-name">Flux Directs Web</div>
            <div class="platform-desc">Flux vidéo HLS (.m3u8) et fichiers directs (.mp4)</div>
          </div>
        </div>
      </div>
    </section>
  {/if}
</main>

<style>
  .app-layout {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 20px 60px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  /* En-tête */
  .app-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-light);
    padding-bottom: 20px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .brand-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 46px;
    border-radius: var(--radius-lg);
    background: linear-gradient(135deg, #2563eb, #3b82f6);
    color: #ffffff;
    box-shadow: 0 4px 10px rgba(37, 99, 235, 0.25);
  }

  .brand-title {
    font-size: 1.4rem;
    font-weight: 700;
    color: var(--text-main);
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .brand-subtitle {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin-top: 2px;
  }

  /* Section input */
  .section-input {
    display: flex;
    justify-content: center;
  }

  /* Aperçu de la vidéo inspectée */
  .section-preview {
    animation: slideUp 250ms ease;
  }

  .video-card {
    display: flex;
    gap: 20px;
    background-color: var(--bg-surface);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-lg);
    padding: 16px;
    box-shadow: var(--shadow-sm);
  }

  .thumbnail-wrapper {
    position: relative;
    width: 220px;
    height: 124px;
    border-radius: var(--radius-md);
    overflow: hidden;
    background-color: var(--bg-surface-muted);
    flex-shrink: 0;
  }

  .thumbnail-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .duration-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background-color: rgba(15, 23, 42, 0.85);
    color: #ffffff;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
  }

  .video-details {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }

  .source-tag {
    display: inline-flex;
    align-self: flex-start;
    padding: 2px 8px;
    background-color: var(--primary-subtle);
    color: var(--primary);
    border: 1px solid var(--primary-border);
    border-radius: var(--radius-full);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .video-title {
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--text-main);
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .video-author {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .formats-summary {
    margin-top: auto;
    font-size: 0.825rem;
    color: var(--text-muted);
  }

  /* Plateformes compatibles */
  .section-supported {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .supported-heading {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .supported-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
  }

  .platform-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px;
    background-color: var(--bg-surface);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-xs);
    transition:
      transform var(--transition-fast),
      box-shadow var(--transition-fast);
  }

  .platform-card:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-sm);
  }

  .platform-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .card-youtube .platform-icon {
    background-color: #fef2f2;
    color: #dc2626;
  }

  .card-movibox .platform-icon {
    background-color: #eef2ff;
    color: #4f46e5;
  }

  .card-generic .platform-icon {
    background-color: #ecfdf5;
    color: #059669;
  }

  .platform-name {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-main);
  }

  .platform-desc {
    font-size: 0.8rem;
    color: var(--text-muted);
    margin-top: 2px;
    line-height: 1.35;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 640px) {
    .video-card {
      flex-direction: column;
    }
    .thumbnail-wrapper {
      width: 100%;
      height: 180px;
    }
  }
</style>
