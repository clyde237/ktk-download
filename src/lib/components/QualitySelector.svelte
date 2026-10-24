<script lang="ts">
  import type { VideoFormat } from '../types/download';
  import { formatBytes } from '../utils/format';

  interface Props {
    options: VideoFormat[];
    value: string;
    onChange: (formatId: string) => void;
    disabled?: boolean;
  }

  let { options, value, onChange, disabled = false }: Props = $props();

  function optionName(option: VideoFormat): string {
    if (option.format_id === 'best') return 'Auto';
    if (option.format_id === 'audio_only') return 'MP3';
    return option.label;
  }

  function optionDetail(option: VideoFormat): string | null {
    const size = option.file_size_bytes ? `≈ ${formatBytes(option.file_size_bytes)}` : null;
    if (option.format_id === 'best' && option.resolution) {
      return size ? `${option.resolution} · ${size}` : option.resolution;
    }
    if (option.format_id === 'audio_only') return 'Audio seul';
    return size;
  }
</script>

<fieldset class="quality" {disabled}>
  <legend class="quality-legend">Qualité</legend>

  <div class="quality-options">
    {#each options as option (option.format_id)}
      {@const detail = optionDetail(option)}
      <label class="quality-option">
        <input
          type="radio"
          name="quality"
          value={option.format_id}
          checked={value === option.format_id}
          onchange={() => onChange(option.format_id)}
        />
        <span class="option-face">
          <span class="option-name">{optionName(option)}</span>
          {#if detail}
            <span class="option-detail">{detail}</span>
          {/if}
        </span>
      </label>
    {/each}
  </div>
</fieldset>

<style>
  .quality {
    border: none;
    min-width: 0;
  }

  .quality-legend {
    margin-bottom: var(--space-2);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
  }

  /* Sélecteur à positions, comme le commutateur de qualité d'un appareil */
  .quality-options {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    padding: var(--space-1);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background-color: var(--color-surface);
  }

  .quality-option {
    position: relative;
    flex: 1 1 0;
    min-width: 72px;
  }

  .quality-option input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: pointer;
  }

  .option-face {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .option-name {
    font-size: var(--text-sm);
    font-weight: 650;
    line-height: 1.3;
  }

  .option-detail {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    line-height: 1.3;
    white-space: nowrap;
  }

  .quality-option:hover .option-face {
    background-color: var(--color-surface-hover);
    color: var(--color-text);
  }

  .quality-option input:checked + .option-face {
    background-color: var(--color-inverse);
    color: var(--color-on-inverse);
  }

  .quality-option input:checked + .option-face .option-detail {
    color: var(--color-on-inverse-muted);
  }

  .quality-option input:focus-visible + .option-face {
    outline: 2px solid var(--color-focus);
    outline-offset: 2px;
  }

  .quality:disabled .quality-option input {
    cursor: not-allowed;
  }

  .quality:disabled .option-face {
    opacity: 0.6;
  }
</style>
