<script lang="ts">
  import { prefersReducedMotion } from 'svelte/motion';
  import { fade, fly } from 'svelte/transition';
  import { toasts, type Toast } from '../stores/toasts.svelte';

  /**
   * Ferme le toast à l'échéance, avec une pause tant qu'il est survolé ou qu'il a le focus,
   * pour laisser le temps de le lire (WCAG 2.2.1).
   */
  function autoDismiss(node: HTMLElement, toast: Toast) {
    if (toast.duration === null) return;

    let remaining = toast.duration;
    let startedAt = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let hovered = false;
    let focused = false;

    const run = () => {
      if (timer || hovered || focused) return;
      startedAt = performance.now();
      timer = setTimeout(() => toasts.dismiss(toast.id), remaining);
    };

    const pause = () => {
      if (!timer) return;
      clearTimeout(timer);
      timer = null;
      remaining -= performance.now() - startedAt;
    };

    const onEnter = () => {
      hovered = true;
      pause();
    };
    const onLeave = () => {
      hovered = false;
      run();
    };
    const onFocusIn = () => {
      focused = true;
      pause();
    };
    const onFocusOut = (event: FocusEvent) => {
      if (node.contains(event.relatedTarget as Node | null)) return;
      focused = false;
      run();
    };

    node.addEventListener('mouseenter', onEnter);
    node.addEventListener('mouseleave', onLeave);
    node.addEventListener('focusin', onFocusIn);
    node.addEventListener('focusout', onFocusOut);
    run();

    return {
      destroy() {
        if (timer) clearTimeout(timer);
        node.removeEventListener('mouseenter', onEnter);
        node.removeEventListener('mouseleave', onLeave);
        node.removeEventListener('focusin', onFocusIn);
        node.removeEventListener('focusout', onFocusOut);
      }
    };
  }

  let enterDuration = $derived(prefersReducedMotion.current ? 0 : 200);
  let exitDuration = $derived(prefersReducedMotion.current ? 0 : 150);
</script>

<!-- Région toujours présente : les lecteurs d'écran n'annoncent que les ajouts dans une région existante -->
<section class="toaster" aria-label="Notifications">
  <ol class="toast-list" aria-live="polite" aria-relevant="additions text">
    {#each toasts.items as toast (toast.id)}
      <li
        class="toast"
        data-kind={toast.kind}
        role={toast.kind === 'error' ? 'alert' : undefined}
        use:autoDismiss={toast}
        in:fly={{ x: 24, duration: enterDuration }}
        out:fade={{ duration: exitDuration }}
      >
        <span class="toast-lamp" aria-hidden="true"></span>
        <div class="toast-body">
          <p class="toast-title">{toast.title}</p>
          {#if toast.message}
            <p class="toast-message">{toast.message}</p>
          {/if}
        </div>
        <button
          type="button"
          class="toast-close"
          onclick={() => toasts.dismiss(toast.id)}
          aria-label="Fermer la notification"
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
      </li>
    {/each}
  </ol>
</section>

<style>
  .toaster {
    position: fixed;
    right: var(--space-4);
    bottom: var(--space-4);
    z-index: 100;
    width: min(360px, calc(100vw - 2 * var(--space-4)));
    pointer-events: none;
  }

  .toast-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    list-style: none;
  }

  /* Afficheur sombre, comme l'écran d'état d'un appareil */
  .toast {
    --lamp-color: var(--color-accent);

    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-2) var(--space-3) var(--space-4);
    border-radius: var(--radius-lg);
    background-color: var(--color-inverse);
    color: var(--color-on-inverse);
    box-shadow:
      0 4px 12px oklch(0.22 0.012 var(--neutral-hue) / 0.18),
      0 1px 3px oklch(0.22 0.012 var(--neutral-hue) / 0.2);
    pointer-events: auto;
  }

  .toast[data-kind='success'] {
    --lamp-color: var(--color-success-meter);
  }

  .toast[data-kind='error'] {
    --lamp-color: var(--color-danger-on-inverse);
  }

  .toast-lamp {
    width: 8px;
    height: 8px;
    margin-top: 6px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
    background-color: var(--lamp-color);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--lamp-color) 25%, transparent);
  }

  .toast-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .toast-title {
    font-size: var(--text-sm);
    font-weight: 650;
    line-height: 1.4;
  }

  .toast-message {
    font-size: var(--text-xs);
    line-height: 1.45;
    color: var(--color-on-inverse-muted);
    overflow-wrap: anywhere;
  }

  .toast-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    color: var(--color-on-inverse-muted);
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .toast-close:hover {
    background-color: oklch(1 0 0 / 0.08);
    color: var(--color-on-inverse);
  }

  @media (max-width: 480px) {
    .toaster {
      right: var(--space-3);
      bottom: var(--space-3);
      width: calc(100vw - 2 * var(--space-3));
    }
  }
</style>
