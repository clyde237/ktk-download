<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { CommandError, ValidatedUrlPayload, VideoMetadata } from '../types/download';
  import { formatErrorMessage } from '../utils/format';

  interface Props {
    onInspect?: (metadata: VideoMetadata) => void;
    onError?: (error: CommandError | null) => void;
    onLoading?: (isLoading: boolean) => void;
    onClear?: () => void;
    disabled?: boolean;
    autofocus?: boolean;
    placeholder?: string;
  }

  let {
    onInspect,
    onError,
    onLoading,
    onClear,
    disabled = false,
    autofocus = false,
    placeholder = 'https://www.youtube.com/watch?v=…'
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

      pasteNotification = 'Lien collé depuis le presse-papier';
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
    onLoading?.(false);
    onError?.(null);
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
    onLoading?.(true);
    onError?.(null);

    try {
      const metadata = await invoke<VideoMetadata>('inspect_url', { url: targetUrl });
      onInspect?.(metadata);
    } catch (err: unknown) {
      const message = formatErrorMessage(err);
      validationError = message;

      const kind =
        typeof err === 'object' &&
        err !== null &&
        'kind' in err &&
        typeof (err as Record<string, unknown>).kind === 'string'
          ? ((err as Record<string, unknown>).kind as CommandError['kind'])
          : 'ExtractionFailed';

      onError?.({
        kind,
        message
      });
    } finally {
      isInspecting = false;
      onLoading?.(false);
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
    <label for="video-url-field" class="input-label">Lien de la vidéo</label>
    <span id="url-formats-hint" class="input-hint"> YouTube, flux HLS (.m3u8), MP4 direct </span>
  </div>

  <!-- Barre de saisie principale -->
  <div
    class="input-wrapper"
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
    >
      {#if isInspecting}
        <span class="spinner" aria-hidden="true"></span>
        <span>Analyse…</span>
      {:else if isValidating}
        <span class="spinner spinner-subtle" aria-hidden="true"></span>
        <span>Vérification…</span>
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
        aria-hidden="true"
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
      <div class="platform-badge">
        {#if validationResult.platform.toLowerCase().includes('youtube')}
          <!-- Logo YouTube SVG -->
          <svg
            class="badge-icon"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"
            />
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
            aria-hidden="true"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="2" y1="12" x2="22" y2="12" />
            <path
              d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
            />
          </svg>
        {/if}
        <span class="badge-text">{validationResult.platform}</span>
        <span class="badge-domain">{validationResult.domain}</span>
      </div>

      <!-- Information de nettoyage d'URL (tracking strip) -->
      {#if isSanitized}
        <div class="sanitized-tag">
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            aria-hidden="true"
          >
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
          </svg>
          <span>Paramètres de pistage retirés</span>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Message d'erreur visible après interaction ou tentative d'analyse -->
  {#if shouldShowError}
    <div id="url-validation-error" class="error-banner" role="alert">
      <svg
        class="error-icon"
        aria-hidden="true"
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
    gap: var(--space-2);
    width: 100%;
    text-align: left;
  }

  .header-group {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: 0 var(--space-1);
  }

  .input-label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
  }

  .input-hint {
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }

  /* Boîtier de saisie */
  .input-wrapper {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-1) var(--space-1) var(--space-4);
    background-color: var(--color-surface);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-panel);
    transition:
      border-color var(--duration-fast) var(--ease-out),
      box-shadow var(--duration-fast) var(--ease-out),
      background-color var(--duration-fast) var(--ease-out);
  }

  .input-wrapper:focus-within {
    border-color: var(--color-accent-strong);
    box-shadow:
      0 0 0 3px oklch(0.64 0.19 var(--signal-hue) / 0.18),
      var(--shadow-panel);
  }

  .input-wrapper.is-valid {
    border-color: var(--color-success);
  }

  .input-wrapper.is-invalid {
    border-color: var(--color-danger);
    background-color: var(--color-danger-wash);
  }

  .input-wrapper.is-invalid:focus-within {
    border-color: var(--color-danger);
    box-shadow:
      0 0 0 3px color-mix(in oklch, var(--color-danger) 22%, transparent),
      var(--shadow-panel);
  }

  .input-wrapper.is-disabled {
    background-color: var(--color-surface-sunken);
    border-color: var(--color-border);
    box-shadow: none;
    pointer-events: none;
  }

  .prefix-icon {
    display: flex;
    color: var(--color-text-subtle);
    flex-shrink: 0;
  }

  .input-wrapper:focus-within .prefix-icon {
    color: var(--color-accent-strong);
  }

  .text-input {
    flex: 1;
    min-width: 0;
    padding: var(--space-2) var(--space-1);
    border: none;
    outline: none;
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--color-text);
    user-select: text;
  }

  .text-input::placeholder {
    color: var(--color-text-subtle);
  }

  .text-input:disabled {
    color: var(--color-text-muted);
  }

  /* Boutons */
  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    flex-shrink: 0;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .clear-btn {
    min-height: 32px;
    padding: var(--space-1);
    border-radius: var(--radius-full);
    color: var(--color-text-subtle);
  }

  .clear-btn:hover {
    color: var(--color-text);
    background-color: var(--color-surface-hover);
  }

  .paste-btn {
    background-color: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    color: var(--color-text-muted);
  }

  .paste-btn:hover:not(:disabled) {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  /* L'orange est réservé à la capture : l'analyse utilise l'encre */
  .inspect-btn {
    padding: var(--space-2) var(--space-4);
    background-color: var(--color-inverse);
    color: var(--color-on-inverse);
    box-shadow: var(--shadow-control);
  }

  .inspect-btn:hover:not(:disabled) {
    background-color: var(--color-inverse-hover);
  }

  .inspect-btn:disabled {
    background-color: var(--color-surface-hover);
    color: var(--color-text-muted);
    box-shadow: none;
    cursor: not-allowed;
  }

  /* Confirmation de collage */
  .toast-indicator {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    align-self: flex-start;
    padding: 2px var(--space-3);
    border-radius: var(--radius-full);
    background-color: var(--color-success-wash);
    color: var(--color-success);
    font-size: var(--text-xs);
    font-weight: 500;
    animation: fade-in var(--duration-base) var(--ease-out);
  }

  /* Plateforme détectée */
  .metadata-status-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding: 0 var(--space-1);
    animation: fade-in var(--duration-base) var(--ease-out);
  }

  .platform-badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 2px var(--space-3) 2px var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-full);
    background-color: var(--color-surface);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color-text);
  }

  .badge-icon {
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .badge-domain {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 400;
    color: var(--color-text-muted);
  }

  .sanitized-tag {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--text-xs);
    color: var(--color-text-muted);
  }

  /* Erreur de saisie */
  .error-banner {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-3);
    border: 1px solid var(--color-danger-border);
    border-radius: var(--radius-md);
    background-color: var(--color-danger-wash);
    color: var(--color-danger);
    font-size: var(--text-sm);
    line-height: 1.4;
    animation: fade-in var(--duration-base) var(--ease-out);
  }

  .error-icon {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .error-text {
    flex: 1;
  }

  /* Indicateurs de chargement */
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid oklch(0.995 0 0 / 0.35);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  .spinner-subtle {
    border-color: oklch(0.46 0.012 var(--neutral-hue) / 0.25);
    border-top-color: currentColor;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes fade-in {
    from {
      opacity: 0;
      transform: translateY(-2px);
    }
  }

  @media (max-width: 600px) {
    .btn-text,
    .input-hint {
      display: none;
    }

    .action-btn {
      padding: var(--space-2);
    }

    .inspect-btn {
      padding: var(--space-2) var(--space-3);
    }
  }
</style>
