export type ToastKind = 'info' | 'success' | 'error';

export interface ToastAction {
  label: string;
  onClick: () => void | Promise<void>;
}

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
  /** Un toast portant la même clé remplace le précédent au lieu de s'empiler. */
  key?: string;
  /** Action interactive optionnelle (ex: « Ouvrir le dossier »). */
  action?: ToastAction;
  /** Durée d'affichage en ms ; `null` : reste affiché jusqu'à sa fermeture. */
  duration: number | null;
}

export interface ToastOptions {
  title: string;
  message?: string;
  key?: string;
  action?: ToastAction;
  duration?: number | null;
}

// Une erreur reste affichée tant qu'elle n'a pas été lue et fermée.
const DEFAULT_DURATIONS: Record<ToastKind, number | null> = {
  info: 4000,
  success: 6000,
  error: null
};

const MAX_VISIBLE_TOASTS = 4;

class ToastStore {
  items = $state<Toast[]>([]);
  #nextId = 1;

  show(kind: ToastKind, options: ToastOptions): number {
    const toast: Toast = {
      id: this.#nextId++,
      kind,
      title: options.title,
      message: options.message,
      key: options.key,
      action: options.action,
      duration: options.duration === undefined ? DEFAULT_DURATIONS[kind] : options.duration
    };

    const sameKeyIndex = options.key ? this.items.findIndex((t) => t.key === options.key) : -1;
    const next =
      sameKeyIndex >= 0 ? this.items.toSpliced(sameKeyIndex, 1, toast) : [...this.items, toast];

    this.items = this.#trim(next);
    return toast.id;
  }

  info(options: ToastOptions) {
    return this.show('info', options);
  }

  success(options: ToastOptions) {
    return this.show('success', options);
  }

  error(options: ToastOptions) {
    return this.show('error', options);
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }

  /** Au-delà de la limite, retire d'abord les plus anciens toasts non bloquants. */
  #trim(list: Toast[]): Toast[] {
    const result = [...list];
    while (result.length > MAX_VISIBLE_TOASTS) {
      const removable = result.findIndex((t) => t.kind !== 'error');
      result.splice(removable >= 0 ? removable : 0, 1);
    }
    return result;
  }
}

export const toasts = new ToastStore();
