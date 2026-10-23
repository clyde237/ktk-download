import type { DownloadEntry } from './downloads.svelte';
import { toasts } from './toasts.svelte';

/** Jalons annoncés pendant le transfert, en pourcentage. */
const PROGRESS_MILESTONES = [25, 50, 75] as const;

function fileNameOf(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/**
 * Traduit l'évolution d'un téléchargement en notifications : une par étape du cycle de vie
 * (départ, jalons, fin, échec, annulation), jamais une par pourcentage reçu.
 *
 * L'état final est notifié seulement quand le résultat de la commande est connu (chemin du
 * fichier, message d'erreur), car l'événement IPC arrive avant ces informations.
 */
export function createDownloadNotifier() {
  let trackedId: string | null = null;
  let startNotified = false;
  let lastMilestone = 0;
  let outcomeNotified = false;

  return function notify(entry: DownloadEntry) {
    if (entry.id !== trackedId) {
      trackedId = entry.id;
      startNotified = false;
      lastMilestone = 0;
      outcomeNotified = false;
    }

    const milestoneKey = `download-milestone:${entry.id}`;

    if (!startNotified) {
      startNotified = true;
      toasts.info({ title: 'Téléchargement lancé', message: entry.title, key: milestoneKey });
    }

    if (entry.status === 'downloading' && entry.progressPercentage < 100) {
      const reached = PROGRESS_MILESTONES.filter((m) => entry.progressPercentage >= m).pop() ?? 0;
      if (reached > lastMilestone) {
        lastMilestone = reached;
        toasts.info({
          title: `${reached} % téléchargés`,
          message: entry.title,
          key: milestoneKey,
          duration: 3000
        });
      }
    }

    if (outcomeNotified) return;

    if (entry.status === 'completed' && entry.filePath) {
      outcomeNotified = true;
      // Remplace le toast de progression : il n'a plus d'intérêt une fois le fichier prêt.
      toasts.success({
        title: 'Téléchargement terminé',
        message: fileNameOf(entry.filePath),
        key: milestoneKey
      });
    } else if (entry.status === 'failed' && entry.errorMessage) {
      outcomeNotified = true;
      toasts.error({
        title: 'Échec du téléchargement',
        message: entry.errorMessage,
        key: milestoneKey
      });
    } else if (entry.status === 'cancelled') {
      outcomeNotified = true;
      toasts.info({ title: 'Téléchargement annulé', message: entry.title, key: milestoneKey });
    }
  };
}
