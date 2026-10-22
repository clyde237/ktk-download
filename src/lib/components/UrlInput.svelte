<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { CommandError, ValidatedUrlPayload, VideoMetadata } from '../types/download';
  import { formatErrorMessage } from '../utils/format';

  interface Props {
    onInspect?: (metadata: VideoMetadata) => void;
    onError?: (error: CommandError | null) => void;
    onClear?: () => void;
    disabled?: boolean;
    autofocus?: boolean;
    placeholder?: string;
  }

  let {
    onInspect,
    onError,
    onClear,
    disabled = false,
    autofocus = false,
    placeholder = 'https://www.youtube.com/watch?v=... ou https://movibox.net/...'
  }: Props = $props();

  // États internes gérés avec les Runes Svelte 5
  let url = $state('');
  let validationResult = $state<ValidatedUrlPayload | null>(null);
  let validationError = $state<string | null>(null);
  let isValidating = $state(false);
  let isInspecting = $state(false);
  let hasInteracted = $state(false);
  let hasBlurred = $state(false);
  let inspectAttempted = $state(false);
  let pasteNotification = $state<string | null>(null);

  let inputElement: HTMLInputElement | null = null;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (autofocus && inputElement) {
      inputElement.focus();
    }
  });

  // Calcul dérivé pour savoir si l'analyse peut être lancée
  let canInspect = $derived(
    url.trim().length > 0 &&
      !isValidating &&
      !isInspecting &&
      !disabled &&
      (validationResult !== null || !validationError)
  );

  // Détection de modification d'URL assainie (paramètres de tracking supprimés)
  let isSanitized = $derived(
    validationResult !== null &&
      url.trim().length > 0 &&
      validationResult.canonical_url !== url.trim()
  );

  // Doit-on afficher l'erreur de validation ? (Conformité :user-invalid après interaction)
  let shouldShowError = $derived(
    Boolean((hasInteracted && hasBlurred) || inspectAttempted) && Boolean(validationError)
  );

  async function runValidation(rawText: string) {
    const trimmed = rawText.trim();
    if (!trimmed) {
      validationResult = null;
      validationError = null;
      isValidating = false;
      return;
    }

    // Vérification rapide du protocole côté client
    if (!trimmed.startsWith('http://') && !trimmed.startsWith('https://')) {
      validationResult = null;
      validationError = 'L\'adresse doit obligatoirement débuter par "http://" ou "https://".';
      isValidating = false;
      return;
    }

    isValidating = true;
    try {
      const result = await invoke<ValidatedUrlPayload>('validate_url', { url: trimmed });
      validationResult = result;
      validationError = null;
    } catch (err: unknown) {
      validationResult = null;
      validationError = formatErrorMessage(err);
    } finally {
      isValidating = false;
    }
  }

  function handleInput(event: Event) {
    const target = event.target as HTMLInputElement;
    url = target.value;
    hasInteracted = true;

    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }

    debounceTimer = setTimeout(() => {
      runValidation(url);
    }, 250);
  }

  function handleBlur() {
    hasBlurred = true;
    if (url.trim().length > 0 && !validationResult && !validationError) {
      runValidation(url);
    }
  }

  async function handlePaste() {
    try {
      const text = await navigator.clipboard.readText();
      const trimmed = text.trim();
      if (!trimmed) return;

      url = trimmed;
      hasInteracted = true;
      hasBlurred = true;
      inspectAttempted = false;

      pasteNotification = 'Lien collé depuis le presse-papier !';
      setTimeout(() => {
        pasteNotification = null;
      }, 2500);

      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
      await runValidation(trimmed);
      inputElement?.focus();
    } catch (err) {
      console.warn('Accès presse-papier restreint :', err);
      inputElement?.focus();
    }
  }

  function handleClear() {
    url = '';
    validationResult = null;
    validationError = null;
    isValidating = false;
    inspectAttempted = false;
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    onClear?.();
    inputElement?.focus();
  }

  async function handleInspect() {
    inspectAttempted = true;
    hasInteracted = true;
    hasBlurred = true;

    const trimmed = url.trim();
    if (!trimmed) {
      validationError = "Veuillez saisir ou coller l'adresse d'une vidéo.";
      inputElement?.focus();
      return;
    }

    // Si pas encore validé, lance la validation d'abord
    if (!validationResult) {
      await runValidation(trimmed);
      if (validationError) return;
    }

    const targetUrl = validationResult?.canonical_url || trimmed;
    isInspecting = true;
    onError?.(null);

    try {
      const metadata = await invoke<VideoMetadata>('inspect_url', { url: targetUrl });
      onInspect?.(metadata);
    } catch (err: unknown) {
      const message = formatErrorMessage(err);
      validationError = message;
      onError?.({
        kind: 'ExtractionFailed',
        message
      });
    } finally {
      isInspecting = false;
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      if (canInspect) {
        handleInspect();
      }
    } else if (event.key === 'Escape') {
      if (url.length > 0) {
        handleClear();
      }
    }
  }
</script>

<div class="url-input-card">
  <!-- En-tête avec label et indication de compatibilité -->
  <div class="header-group">
    <label for="video-url-field" class="input-label"> URL de la vidéo </label>
    <span id="url-formats-hint" class="input-hint">
      YouTube, MoviBox (movibox.net), flux HLS (.m3u8), MP4 direct
    </span>
  </div>

  <!-- Barre de saisie principale -->
  <div
    class="input-wrapper"
    class:is-focused={false}
    class:is-valid={validationResult !== null}
    class:is-invalid={shouldShowError}
    class:is-disabled={disabled || isInspecting}
  >
    <!-- Icône préfixe -->
    <div class="prefix-icon" aria-hidden="true">
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
        <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
      </svg>
    </div>

    <!-- Champ de texte accessible -->
    <input
      id="video-url-field"
      bind:this={inputElement}
      type="url"
      inputmode="url"
      autocomplete="url"
      spellcheck="false"
      value={url}
      {placeholder}
      disabled={disabled || isInspecting}
      aria-describedby="url-formats-hint {shouldShowError ? 'url-validation-error' : ''}"
      aria-invalid={shouldShowError ? 'true' : 'false'}
      oninput={handleInput}
      onblur={handleBlur}
      onkeydown={handleKeyDown}
      class="text-input"
    />

    <!-- Bouton Effacer le texte -->
    {#if url.length > 0 && !isInspecting}
      <button
        type="button"
        class="action-btn clear-btn"
        onclick={handleClear}
        title="Effacer le lien (Échap)"
        aria-label="Effacer le lien"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="10" />
          <line x1="15" y1="9" x2="9" y2="15" />
          <line x1="9" y1="9" x2="15" y2="15" />
        </svg>
      </button>
    {/if}

    <!-- Bouton Coller depuis le presse-papier -->
    <button
      type="button"
      class="action-btn paste-btn"
      onclick={handlePaste}
      disabled={disabled || isInspecting}
      title="Coller depuis le presse-papier"
      aria-label="Coller le lien copié"
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <rect x="9" y="2" width="6" height="4" rx="1" ry="1" />
        <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
      </svg>
      <span class="btn-text">Coller</span>
    </button>

    <!-- Bouton Analyser / Inspecter -->
    <button
      type="button"
      class="action-btn inspect-btn"
      onclick={handleInspect}
      disabled={!canInspect}
      title="Analyser et charger les informations de la vidéo (Entrée)"
      aria-label="Analyser la vidéo"
    >
      {#if isInspecting}
        <span class="spinner" aria-hidden="true"></span>
        <span>Analyse en cours...</span>
      {:else if isValidating}
        <span class="spinner spinner-subtle" aria-hidden="true"></span>
        <span>Vérification...</span>
      {:else}
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
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <span>Analyser</span>
      {/if}
    </button>
  </div>

  <!-- Notification contextuelle de collage -->
  {#if pasteNotification}
    <div class="toast-indicator" role="status">
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
      >
        <polyline points="20 6 9 17 4 12" />
      </svg>
      <span>{pasteNotification}</span>
    </div>
  {/if}

  <!-- Badge de détection de plateforme et information d'assainissement -->
  {#if validationResult}
    <div class="metadata-status-bar">
      <!-- Badge de plateforme reconnue -->
      <div
        class="platform-badge"
        class:badge-youtube={validationResult.platform.toLowerCase().includes('youtube')}
        class:badge-movibox={validationResult.platform.toLowerCase().includes('movibox')}
        class:badge-tiktok={validationResult.platform.toLowerCase().includes('tiktok')}
        class:badge-twitter={validationResult.platform.toLowerCase().includes('twitter')}
        class:badge-generic={validationResult.platform.toLowerCase().includes('web') ||
          validationResult.platform.toLowerCase().includes('direct')}
      >
        {#if validationResult.platform.toLowerCase().includes('youtube')}
          <!-- Logo YouTube SVG -->
          <svg class="badge-icon" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <path
              d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"
            />
          </svg>
        {:else if validationResult.platform.toLowerCase().includes('movibox')}
          <!-- Logo MoviBox Film SVG -->
          <svg
            class="badge-icon"
            width="14"
            height="14"
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
        {:else}
          <!-- Logo Flux Web SVG -->
          <svg
            class="badge-icon"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="2" y1="12" x2="22" y2="12" />
            <path
              d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
            />
          </svg>
        {/if}
        <span class="badge-text">{validationResult.platform}</span>
        <span class="badge-domain">({validationResult.domain})</span>
      </div>

      <!-- Information de nettoyage d'URL (tracking strip) -->
      {#if isSanitized}
        <div class="sanitized-tag" title="Paramètres d'analyse et de pistage retirés">
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
          >
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
          </svg>
          <span>URL nettoyée et sécurisée</span>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Message d'erreur visible après interaction ou tentative d'analyse -->
  {#if shouldShowError}
    <div id="url-validation-error" class="error-banner" role="alert">
      <svg
        class="error-icon"
        width="16"
        height="16"
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
      <span class="error-text">{validationError}</span>
    </div>
  {/if}
</div>

<style>
  .url-input-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    max-width: 780px;
    margin: 0 auto;
    text-align: left;
  }

  .header-group {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 4px;
  }

  .input-label {
    font-size: 0.925rem;
    font-weight: 600;
    color: var(--text-main);
  }

  .input-hint {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Boîtier principal de l'input */
  .input-wrapper {
    display: flex;
    align-items: center;
    background-color: var(--bg-surface);
    border: 1.5px solid var(--border-light);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    padding: 4px 6px 4px 14px;
    gap: 8px;
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast),
      background-color var(--transition-fast);
  }

  .input-wrapper:focus-within {
    border-color: var(--border-focus);
    box-shadow:
      0 0 0 3px rgba(37, 99, 235, 0.15),
      var(--shadow-sm);
  }

  .input-wrapper.is-valid {
    border-color: var(--success-border);
  }

  .input-wrapper.is-invalid {
    border-color: var(--error);
    background-color: var(--error-subtle);
  }

  .input-wrapper.is-disabled {
    opacity: 0.7;
    background-color: var(--bg-surface-muted);
    pointer-events: none;
  }

  .prefix-icon {
    display: flex;
    align-items: center;
    color: var(--text-subtle);
    flex-shrink: 0;
  }

  .input-wrapper:focus-within .prefix-icon {
    color: var(--primary);
  }

  .text-input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    font-size: 0.95rem;
    color: var(--text-main);
    padding: 8px 4px;
  }

  .text-input::placeholder {
    color: var(--text-subtle);
    font-size: 0.9rem;
  }

  /* Boutons d'action */
  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: var(--radius-md);
    font-size: 0.875rem;
    font-weight: 500;
    transition: all var(--transition-fast);
    flex-shrink: 0;
    user-select: none;
  }

  .clear-btn {
    padding: 6px;
    border-radius: var(--radius-full);
    color: var(--text-subtle);
  }

  .clear-btn:hover {
    color: var(--text-main);
    background-color: var(--bg-surface-hover);
  }

  .paste-btn {
    background-color: var(--bg-surface-muted);
    color: var(--text-muted);
    border: 1px solid var(--border-light);
  }

  .paste-btn:hover:not(:disabled) {
    background-color: var(--bg-surface-hover);
    color: var(--text-main);
  }

  .inspect-btn {
    background-color: var(--primary);
    color: var(--text-inverse);
    font-weight: 600;
    padding: 8px 16px;
    box-shadow: var(--shadow-xs);
  }

  .inspect-btn:hover:not(:disabled) {
    background-color: var(--primary-hover);
    box-shadow: var(--shadow-sm);
  }

  .inspect-btn:active:not(:disabled) {
    background-color: var(--primary-active);
  }

  .inspect-btn:disabled {
    background-color: var(--text-subtle);
    opacity: 0.65;
    cursor: not-allowed;
  }

  /* Notification discrète de collage */
  .toast-indicator {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8rem;
    color: var(--success);
    background-color: var(--success-subtle);
    padding: 3px 10px;
    border-radius: var(--radius-full);
    border: 1px solid var(--success-border);
    align-self: flex-start;
    animation: fadeIn 200ms ease;
  }

  /* Barre d'état de détection de plateforme */
  .metadata-status-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 4px;
    flex-wrap: wrap;
    animation: fadeIn 200ms ease;
  }

  .platform-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: var(--radius-full);
    font-size: 0.825rem;
    font-weight: 600;
    background-color: var(--bg-surface-muted);
    color: var(--text-main);
    border: 1px solid var(--border-light);
  }

  .badge-icon {
    flex-shrink: 0;
  }

  .badge-youtube {
    background-color: #fef2f2;
    border-color: #fecaca;
    color: #dc2626;
  }

  .badge-movibox {
    background-color: #eef2ff;
    border-color: #c7d2fe;
    color: #4f46e5;
  }

  .badge-tiktok {
    background-color: #fff1f2;
    border-color: #fecdd3;
    color: #e11d48;
  }

  .badge-twitter {
    background-color: #f8fafc;
    border-color: #cbd5e1;
    color: #0f172a;
  }

  .badge-generic {
    background-color: #ecfdf5;
    border-color: #a7f3d0;
    color: #059669;
  }

  .badge-domain {
    font-weight: 400;
    font-size: 0.775rem;
    opacity: 0.8;
  }

  .sanitized-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.775rem;
    color: var(--text-muted);
    background-color: var(--bg-surface-muted);
    padding: 3px 8px;
    border-radius: var(--radius-full);
    border: 1px solid var(--border-light);
  }

  /* Bannière d'erreur */
  .error-banner {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--radius-md);
    background-color: var(--error-subtle);
    border: 1px solid var(--error-border);
    color: var(--error);
    font-size: 0.85rem;
    line-height: 1.4;
    animation: fadeIn 200ms ease;
  }

  .error-icon {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .error-text {
    flex: 1;
  }

  /* Spinners de chargement */
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.4);
    border-radius: 50%;
    border-top-color: #ffffff;
    animation: spin 700ms linear infinite;
  }

  .spinner-subtle {
    border-color: rgba(37, 99, 235, 0.2);
    border-top-color: var(--primary);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 600px) {
    .btn-text {
      display: none;
    }
    .input-hint {
      display: none;
    }
    .action-btn {
      padding: 7px 9px;
    }
  }
</style>
