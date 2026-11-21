<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import type { DependencyProgressPayload, DependencyStatus } from '$lib/types/download';
  import { formatBytes } from '$lib/utils/format';

  interface Props {
    isOpen: boolean;
    onClose: () => void;
    onSuccess: () => void;
    reason?: string | null;
  }

  let { isOpen, onClose, onSuccess, reason = null }: Props = $props();

  let isInstalling = $state(false);
  let progress = $state<DependencyProgressPayload | null>(null);
  let errorMessage = $state<string | null>(null);
  let isCompleted = $state(false);
  let status = $state<DependencyStatus | null>(null);

  onMount(() => {
    let unlisten: UnlistenFn | undefined;

    void listen<DependencyProgressPayload>('dependency-progress', (event) => {
      progress = event.payload;
      if (event.payload.stage === 'ready' && event.payload.percentage === 100) {
        // Un composant est prêt
      }
    }).then((fn) => {
      unlisten = fn;
    });

    void refreshStatus();

    return () => {
      if (unlisten) unlisten();
    };
  });

  async function refreshStatus() {
    try {
      status = await invoke<DependencyStatus>('check_system_dependencies');
    } catch (e) {
      console.error('Erreur lors de la vérification des dépendances :', e);
    }
  }

  async function handleInstall() {
    if (isInstalling) return;
    isInstalling = true;
    errorMessage = null;
    isCompleted = false;

    try {
      const res = await invoke<DependencyStatus>('install_system_dependencies');
      status = res;
      isCompleted = true;
      setTimeout(() => {
        onSuccess();
      }, 1200);
    } catch (err: unknown) {
      console.error("Échec d'installation des dépendances :", err);
      errorMessage =
        typeof err === 'string'
          ? err
          : (err as { message?: string })?.message ||
            "Une erreur s'est produite lors du téléchargement des composants.";
    } finally {
      isInstalling = false;
    }
  }

  let formattedDownload = $derived.by(() => {
    if (!progress || !progress.bytes_downloaded) return null;
    const downloaded = formatBytes(progress.bytes_downloaded);
    if (progress.total_bytes && progress.total_bytes > 0) {
      const total = formatBytes(progress.total_bytes);
      return `${downloaded} / ${total}`;
    }
    return downloaded;
  });
</script>

{#if isOpen}
  <div
    class="modal-backdrop"
    role="dialog"
    aria-modal="true"
    aria-labelledby="dep-modal-title"
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key === 'Escape' && !isInstalling) onClose();
    }}
  >
    <div class="modal-card">
      <header class="modal-header">
        <div class="modal-icon-badge" class:is-ready={isCompleted}>
          {#if isCompleted}
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          {:else}
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path
                d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"
              />
              <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
              <line x1="12" y1="22.08" x2="12" y2="12" />
            </svg>
          {/if}
        </div>
        <div class="modal-header-text">
          <h2 id="dep-modal-title" class="modal-title">
            {isCompleted ? 'Composants prêts !' : 'Installation des composants requis'}
          </h2>
          <p class="modal-subtitle">
            {isCompleted
              ? 'yt-dlp et FFmpeg sont configurés et opérationnels'
              : 'Outils haute performance nécessaires pour YouTube et le web'}
          </p>
        </div>
        {#if !isInstalling}
          <button type="button" class="modal-close-btn" onclick={onClose} aria-label="Fermer">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        {/if}
      </header>

      <div class="modal-body">
        {#if reason}
          <div class="reason-alert">
            <span class="reason-tag">Plateforme détectée</span>
            <p class="reason-text">{reason}</p>
          </div>
        {/if}

        <p class="modal-desc">
          Pour analyser les flux vidéo sécurisés et convertir l’audio en MP3, <strong
            >ktk-download</strong
          >
          utilise les moteurs de référence <strong>yt-dlp</strong> et <strong>FFmpeg</strong>.
        </p>

        <div class="components-grid">
          <div class="component-card">
            <div class="component-info">
              <span class="component-name">yt-dlp</span>
              <span class="component-role">Moteur d’extraction et flux vidéo</span>
            </div>
            <span
              class="component-badge"
              class:badge-success={status?.ytdlp_installed || isCompleted}
              class:badge-warning={!status?.ytdlp_installed && !isCompleted}
            >
              {status?.ytdlp_installed || isCompleted ? '✓ Présent' : 'À installer'}
            </span>
          </div>

          <div class="component-card">
            <div class="component-info">
              <span class="component-name">FFmpeg</span>
              <span class="component-role">Fusion audio/vidéo 1080p+ et conversion MP3</span>
            </div>
            <span
              class="component-badge"
              class:badge-success={status?.ffmpeg_installed || isCompleted}
              class:badge-warning={!status?.ffmpeg_installed && !isCompleted}
            >
              {status?.ffmpeg_installed || isCompleted ? '✓ Présent' : 'À installer'}
            </span>
          </div>
        </div>

        {#if isInstalling}
          <div class="progress-box">
            <div class="progress-header">
              <span class="progress-label">{progress?.message || 'Téléchargement en cours...'}</span
              >
              {#if progress?.percentage}
                <span class="progress-val">{progress.percentage}%</span>
              {/if}
            </div>
            <div class="progress-bar-track">
              <div class="progress-bar-fill" style="width: {progress?.percentage ?? 15}%"></div>
            </div>
            {#if formattedDownload}
              <span class="progress-bytes">{formattedDownload}</span>
            {/if}
          </div>
        {/if}

        {#if errorMessage}
          <div class="error-box" role="alert">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            <span>{errorMessage}</span>
          </div>
        {/if}

        {#if isCompleted}
          <div class="success-box" role="status">
            <span>Installation terminée avec succès ! Reprise de l'analyse...</span>
          </div>
        {/if}
      </div>

      <footer class="modal-footer">
        {#if !isCompleted}
          <button type="button" class="modal-btn-cancel" onclick={onClose} disabled={isInstalling}>
            Annuler
          </button>
          <button
            type="button"
            class="modal-btn-confirm"
            onclick={handleInstall}
            disabled={isInstalling}
          >
            {#if isInstalling}
              <span class="btn-spinner" aria-hidden="true"></span>
              <span>Installation en cours…</span>
            {:else}
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.2"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              <span>Installer automatiquement</span>
            {/if}
          </button>
        {:else}
          <button type="button" class="modal-btn-confirm" onclick={onSuccess}> Continuer </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1100;
    background-color: rgba(15, 23, 42, 0.45);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    animation: fadeIn 0.15s ease-out;
  }

  .modal-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg, 12px);
    box-shadow:
      0 20px 25px -5px rgba(0, 0, 0, 0.1),
      0 8px 10px -6px rgba(0, 0, 0, 0.05);
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modalPop 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-sunken);
  }

  .modal-icon-badge {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--color-accent-wash);
    color: var(--color-accent-strong);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.2s ease;
  }

  .modal-icon-badge.is-ready {
    background: var(--color-success-wash);
    color: var(--color-success);
  }

  .modal-header-text {
    flex: 1;
    min-width: 0;
  }

  .modal-title {
    font-size: var(--text-base);
    font-weight: 700;
    color: var(--color-text);
    margin: 0;
    line-height: 1.25;
  }

  .modal-subtitle {
    font-size: var(--text-xs);
    color: var(--color-text-muted);
    margin-top: 2px;
  }

  .modal-close-btn {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: var(--radius-md, 6px);
    color: var(--color-text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background-color 0.15s ease,
      color 0.15s ease;
  }

  .modal-close-btn:hover {
    background: var(--color-surface-hover);
    color: var(--color-text);
  }

  .modal-body {
    padding: var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .reason-alert {
    background: #fffbeb;
    border: 1px solid #fef3c7;
    border-radius: var(--radius-md, 6px);
    padding: 8px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .reason-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    background: #fde68a;
    color: #92400e;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .reason-text {
    font-size: var(--text-xs);
    color: #78350f;
    margin: 0;
  }

  .modal-desc {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
    line-height: 1.5;
    margin: 0;
  }

  .components-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .component-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md, 6px);
  }

  .component-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .component-name {
    font-size: var(--text-sm);
    font-weight: 700;
    color: var(--color-text);
  }

  .component-role {
    font-size: var(--text-xs);
    color: var(--color-text-subtle);
  }

  .component-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 9999px;
  }

  .badge-success {
    background: var(--color-success-wash);
    color: var(--color-success);
    border: 1px solid rgba(22, 163, 74, 0.2);
  }

  .badge-warning {
    background: #fef3c7;
    color: #b45309;
    border: 1px solid #fde68a;
  }

  .progress-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md, 6px);
  }

  .progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-text);
  }

  .progress-bar-track {
    height: 8px;
    background: var(--color-border);
    border-radius: 9999px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--color-accent);
    border-radius: 9999px;
    transition: width 0.2s ease-out;
  }

  .progress-bytes {
    font-size: 11px;
    color: var(--color-text-subtle);
    font-family: var(--font-mono);
    text-align: right;
  }

  .error-box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: #fef2f2;
    border: 1px solid #fee2e2;
    border-radius: var(--radius-md, 6px);
    font-size: var(--text-xs);
    color: var(--color-danger);
  }

  .success-box {
    padding: 10px 12px;
    background: var(--color-success-wash);
    border: 1px solid rgba(22, 163, 74, 0.2);
    border-radius: var(--radius-md, 6px);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-success);
    text-align: center;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-top: 1px solid var(--color-border);
    background: var(--color-surface-sunken);
  }

  .modal-btn-cancel {
    background: transparent;
    border: 1px solid var(--color-border);
    color: var(--color-text);
    border-radius: var(--radius-md, 6px);
    padding: 8px 16px;
    font-size: var(--text-sm);
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .modal-btn-cancel:hover {
    background: var(--color-surface-hover);
  }

  .modal-btn-confirm {
    background: var(--color-accent);
    color: var(--color-on-accent);
    border: none;
    border-radius: var(--radius-md, 6px);
    padding: 8px 18px;
    font-size: var(--text-sm);
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    transition:
      background-color 0.15s ease,
      transform 0.05s ease;
  }

  .modal-btn-confirm:hover {
    background: var(--color-accent-strong);
  }

  .modal-btn-confirm:active {
    transform: scale(0.98);
  }

  .modal-btn-confirm:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes modalPop {
    from {
      opacity: 0;
      transform: scale(0.95) translateY(6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }
</style>
