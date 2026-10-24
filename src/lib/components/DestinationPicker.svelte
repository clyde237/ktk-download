<script lang="ts">
  import { destination } from '../stores/destination.svelte';
  import { openFolder } from '../utils/filesystem';
  import { formatErrorMessage } from '../utils/format';

  interface Props {
    disabled?: boolean;
  }

  let { disabled = false }: Props = $props();

  // Valeur sentinelle du <select> pour le dossier par défaut (aucun chemin envoyé au backend).
  const DEFAULT_OPTION = '__default__';

  let browseError = $state<string | null>(null);

  $effect(() => {
    destination.resolveSystemFolders();
  });

  let defaultLabel = $derived(
    destination.defaultFolder
      ? `${destination.displayPath(destination.defaultFolder)} (par défaut)`
      : 'Téléchargements (par défaut)'
  );

  // Dossiers récents, sans doublon avec le dossier par défaut
  let recentFolders = $derived(
    destination.recent.filter((folder) => folder !== destination.defaultFolder)
  );

  // Un dossier choisi via « Parcourir… » mais pas encore utilisé doit rester visible dans la liste.
  let showSelectedOutsideRecent = $derived(
    destination.selected !== null &&
      destination.selected !== destination.defaultFolder &&
      !recentFolders.includes(destination.selected)
  );

  let selectValue = $derived(
    destination.selected === null || destination.selected === destination.defaultFolder
      ? DEFAULT_OPTION
      : destination.selected
  );

  function handleChange(event: Event) {
    const value = (event.target as HTMLSelectElement).value;
    destination.select(value === DEFAULT_OPTION ? null : value);
    browseError = null;
  }

  async function handleBrowse() {
    browseError = null;
    try {
      await destination.browse();
    } catch (err: unknown) {
      browseError = `Impossible d'ouvrir le sélecteur de dossier : ${formatErrorMessage(err)}`;
    }
  }

  async function handleOpenFolder() {
    const target = destination.selected ?? destination.defaultFolder;
    if (target) {
      await openFolder(target, destination.displayPath(target));
    }
  }
</script>

<div class="destination">
  <label for="destination-select" class="destination-label">Enregistrer dans</label>

  <div class="destination-controls">
    <div class="select-wrap" class:is-disabled={disabled}>
      <svg
        class="folder-icon"
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>

      <select
        id="destination-select"
        class="folder-select"
        value={selectValue}
        onchange={handleChange}
        {disabled}
        title={destination.selected ?? destination.defaultFolder ?? undefined}
      >
        <option value={DEFAULT_OPTION}>{defaultLabel}</option>
        {#if showSelectedOutsideRecent && destination.selected}
          <option value={destination.selected}
            >{destination.displayPath(destination.selected)}</option
          >
        {/if}
        {#if recentFolders.length > 0}
          <optgroup label="Dossiers récents">
            {#each recentFolders as folder (folder)}
              <option value={folder}>{destination.displayPath(folder)}</option>
            {/each}
          </optgroup>
        {/if}
      </select>

      <svg
        class="chevron-icon"
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
    </div>

    <button type="button" class="browse-btn" onclick={handleBrowse} {disabled}> Parcourir… </button>

    <button
      type="button"
      class="browse-btn open-folder-btn"
      onclick={handleOpenFolder}
      disabled={disabled || (!destination.selected && !destination.defaultFolder)}
      title="Ouvrir le dossier dans le gestionnaire de fichiers"
      aria-label="Ouvrir le dossier de destination dans le gestionnaire de fichiers"
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
        <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
        <polyline points="15 3 21 3 21 9" />
        <line x1="10" y1="14" x2="21" y2="3" />
      </svg>
      <span>Ouvrir</span>
    </button>
  </div>

  {#if browseError}
    <div class="destination-error-banner" role="alert">
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
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
      <span class="destination-error-text">{browseError}</span>
      <button
        type="button"
        class="destination-error-dismiss"
        onclick={() => (browseError = null)}
        aria-label="Fermer l'erreur"
      >
        ×
      </button>
    </div>
  {/if}
</div>

<style>
  .destination {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    flex: 1;
  }

  .destination-label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
  }

  .destination-controls {
    display: flex;
    gap: var(--space-2);
    min-width: 0;
  }

  .select-wrap {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .folder-icon,
  .chevron-icon {
    position: absolute;
    color: var(--color-text-muted);
    pointer-events: none;
  }

  .folder-icon {
    left: var(--space-3);
  }

  .chevron-icon {
    right: var(--space-3);
  }

  .folder-select {
    width: 100%;
    min-height: 36px;
    padding: var(--space-2) var(--space-8) var(--space-2) var(--space-10);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    background-color: var(--color-surface);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-text);
    text-overflow: ellipsis;
    appearance: none;
    cursor: pointer;
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  .folder-select:hover:not(:disabled) {
    border-color: var(--color-text-muted);
  }

  .folder-select:disabled {
    border-color: var(--color-border);
    background-color: var(--color-surface-sunken);
    color: var(--color-text-muted);
    cursor: not-allowed;
  }

  .browse-btn {
    min-height: 36px;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background-color: var(--color-surface-sunken);
    color: var(--color-text);
    font-size: var(--text-sm);
    font-weight: 600;
    flex-shrink: 0;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .browse-btn:hover:not(:disabled) {
    background-color: var(--color-surface-hover);
  }

  .browse-btn:disabled {
    color: var(--color-text-muted);
    cursor: not-allowed;
  }

  .open-folder-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .destination-error-banner {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    background-color: var(--color-danger-wash);
    border: 1px solid var(--color-danger-border);
    color: var(--color-danger);
    font-size: var(--text-xs);
  }

  .destination-error-text {
    flex: 1;
    line-height: 1.4;
  }

  .destination-error-dismiss {
    font-size: var(--text-base);
    line-height: 1;
    color: var(--color-danger);
    padding: 0 var(--space-1);
    cursor: pointer;
  }
</style>
