import { isRunningStatus, type DownloadEntry } from './downloads.svelte';
import { toasts } from './toasts.svelte';

/** Jalons annoncés pendant le transfert, en pourcentage. */
const PROGRESS_MILESTONES = [25, 50, 75] as const;

interface NotificationTracking {
  queueNotified: boolean;
  startNotified: boolean;
  lastMilestone: number;
  outcomeNotified: boolean;
}

function fileNameOf(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/**
 * Traduit l'évolution des téléchargements de la file en notifications : une par étape du cycle
 * de vie (mise en file, départ, jalons, fin, échec, annulation), jamais une par pourcentage.
 * Chaque téléchargement a son propre emplacement de toast, remplacé à chaque étape.
 *
 * L'état final est notifié seulement quand le résultat de la commande est connu (chemin du
 * fichier, message d'erreur), car l'événement IPC arrive avant ces informations. Une pause et
 * sa reprise, décidées par l'utilisateur, ne sont pas notifiées.
 */
export function createDownloadNotifier() {
  const tracked = new Map<string, NotificationTracking>();

  return function notify(entry: DownloadEntry) {
    let tracking = tracked.get(entry.id);
    if (!tracking) {
      tracking = {
        queueNotified: false,
        startNotified: false,
        lastMilestone: 0,
        outcomeNotified: false
      };
      tracked.set(entry.id, tracking);
    }
    if (tracking.outcomeNotified) return;

    const key = `download:${entry.id}`;

    if (entry.status === 'waiting' && !tracking.queueNotified && !tracking.startNotified) {
      tracking.queueNotified = true;
      toasts.info({ title: "Ajouté à la file d'attente", message: entry.title, key });
    }

    if (isRunningStatus(entry.status) && !tracking.startNotified) {
      tracking.startNotified = true;
      toasts.info({ title: 'Téléchargement lancé', message: entry.title, key });
    }

    if (entry.status === 'downloading' && entry.progressPercentage < 100) {
      const reached = PROGRESS_MILESTONES.filter((m) => entry.progressPercentage >= m).pop() ?? 0;
      if (reached > tracking.lastMilestone) {
        tracking.lastMilestone = reached;
        toasts.info({
          title: `${reached} % téléchargés`,
          message: entry.title,
          key,
          duration: 3000
        });
      }
    }

    if (entry.status === 'completed' && entry.filePath) {
      tracking.outcomeNotified = true;
      toasts.success({
        title: 'Téléchargement terminé',
        message: fileNameOf(entry.filePath),
        key
      });
    } else if (entry.status === 'failed' && entry.errorMessage) {
      tracking.outcomeNotified = true;
      toasts.error({ title: 'Échec du téléchargement', message: entry.errorMessage, key });
    } else if (entry.status === 'cancelled') {
      tracking.outcomeNotified = true;
      toasts.info({ title: 'Téléchargement annulé', message: entry.title, key });
    }
  };
}
