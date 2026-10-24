/**
 * ktk-download — Popup Logic
 * Compatible Chrome, Edge, Firefox, Opera.
 */

const browserAPI = globalThis.browser ?? globalThis.chrome;

let currentTabId = null;
let currentMediaList = [];

/**
 * Formate une taille en octets en chaîne lisible (Ko, Mo, Go).
 */
function formatBytes(bytes) {
  if (!bytes || isNaN(bytes) || bytes <= 0) return null;
  const units = ['o', 'Ko', 'Mo', 'Go'];
  let i = 0;
  let val = bytes;
  while (val >= 1024 && i < units.length - 1) {
    val /= 1024;
    i++;
  }
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/**
 * Vérifie le statut de connexion au serveur local ktk-download.
 */
async function updateAppStatus() {
  const statusDot = document.getElementById('status-dot');
  const statusLabel = document.getElementById('status-label');

  try {
    const res = await browserAPI.runtime.sendMessage({ type: 'CHECK_APP_STATUS' });
    if (res && res.connected) {
      statusDot.className = 'status-dot status-online';
      statusLabel.textContent = 'Connecté';
    } else {
      statusDot.className = 'status-dot status-offline';
      statusLabel.textContent = 'App fermée';
    }
  } catch (_e) {
    statusDot.className = 'status-dot status-offline';
    statusLabel.textContent = 'Non connecté';
  }
}

/**
 * Affiche la liste des médias détectés.
 */
function renderMediaList(mediaList) {
  currentMediaList = mediaList;
  const listEl = document.getElementById('media-list');
  const emptyEl = document.getElementById('empty-state');
  const actionBar = document.getElementById('action-bar');
  const counterEl = document.getElementById('media-counter');

  listEl.innerHTML = '';

  if (!mediaList || mediaList.length === 0) {
    emptyEl.style.display = 'flex';
    actionBar.style.display = 'none';
    return;
  }

  emptyEl.style.display = 'none';
  actionBar.style.display = 'flex';
  const count = mediaList.length;
  counterEl.textContent = `${count} vidéo${count > 1 ? 's' : ''} détectée${count > 1 ? 's' : ''}`;

  mediaList.forEach((media) => {
    const card = document.createElement('div');
    card.className = 'media-card';

    const header = document.createElement('div');
    header.className = 'media-card-header';

    const titleEl = document.createElement('div');
    titleEl.className = 'media-title';
    titleEl.textContent = media.title || 'Vidéo web';
    titleEl.title = media.title || media.url;

    header.appendChild(titleEl);

    const badges = document.createElement('div');
    badges.className = 'media-badges';

    const formatBadge = document.createElement('span');
    formatBadge.className = 'badge badge-format';
    formatBadge.textContent = media.format || 'Vidéo';
    badges.appendChild(formatBadge);

    if (media.resolution) {
      const resBadge = document.createElement('span');
      resBadge.className = 'badge';
      resBadge.textContent = media.resolution;
      badges.appendChild(resBadge);
    }

    if (media.sizeBytes) {
      const sizeStr = formatBytes(media.sizeBytes);
      if (sizeStr) {
        const sizeBadge = document.createElement('span');
        sizeBadge.className = 'badge';
        sizeBadge.textContent = sizeStr;
        badges.appendChild(sizeBadge);
      }
    }

    const actions = document.createElement('div');
    actions.className = 'media-card-actions';

    const downloadBtn = document.createElement('button');
    downloadBtn.className = 'btn btn-primary btn-sm';
    downloadBtn.innerHTML = `
      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
        <path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM17 13l-5 5-5-5h3V9h4v4h3z"/>
      </svg>
      <span>Télécharger</span>
    `;

    downloadBtn.addEventListener('click', async () => {
      downloadBtn.disabled = true;
      downloadBtn.innerHTML = `<span>Envoi...</span>`;
      try {
        const res = await browserAPI.runtime.sendMessage({
          type: 'TRIGGER_DOWNLOAD',
          payload: {
            url: media.url,
            title: media.title,
            formatId: media.resolution ? `${media.resolution}` : 'best'
          }
        });
        if (res && res.success) {
          downloadBtn.innerHTML = `<span>✓ Ajouté</span>`;
        } else {
          downloadBtn.innerHTML = `<span>Échec</span>`;
          downloadBtn.title = res?.error || 'Erreur inconnue';
        }
      } catch (_e) {
        downloadBtn.innerHTML = `<span>Erreur</span>`;
      } finally {
        setTimeout(() => {
          downloadBtn.disabled = false;
          downloadBtn.innerHTML = `<span>Télécharger</span>`;
        }, 2000);
      }
    });

    actions.appendChild(downloadBtn);

    card.appendChild(header);
    card.appendChild(badges);
    card.appendChild(actions);
    listEl.appendChild(card);
  });
}

/**
 * Charge les médias pour l'onglet actif.
 */
async function loadTabMedia() {
  const tabs = await browserAPI.tabs.query({ active: true, currentWindow: true });
  if (!tabs || tabs.length === 0) return;

  currentTabId = tabs[0].id;
  const res = await browserAPI.runtime.sendMessage({
    type: 'GET_TAB_MEDIA',
    tabId: currentTabId
  });

  renderMediaList(res?.media || []);
}

document.addEventListener('DOMContentLoaded', async () => {
  await updateAppStatus();
  await loadTabMedia();

  // Bouton Tout Télécharger
  const btnDownloadAll = document.getElementById('btn-download-all');
  btnDownloadAll.addEventListener('click', async () => {
    if (!currentMediaList.length) return;
    btnDownloadAll.disabled = true;
    btnDownloadAll.textContent = 'Envoi...';

    for (const item of currentMediaList) {
      await browserAPI.runtime.sendMessage({
        type: 'TRIGGER_DOWNLOAD',
        payload: {
          url: item.url,
          title: item.title,
          formatId: item.resolution ? `${item.resolution}` : 'best'
        }
      });
    }

    btnDownloadAll.textContent = '✓ Transmis !';
    setTimeout(() => {
      btnDownloadAll.disabled = false;
      btnDownloadAll.textContent = 'Tout télécharger';
    }, 2000);
  });

  // Bouton Effacer
  const btnClear = document.getElementById('btn-clear');
  btnClear.addEventListener('click', async () => {
    if (!currentTabId) return;
    await browserAPI.runtime.sendMessage({
      type: 'CLEAR_TAB_MEDIA',
      tabId: currentTabId
    });
    renderMediaList([]);
  });

  // Lien pour ouvrir l'application
  const linkOpenApp = document.getElementById('link-open-app');
  linkOpenApp.addEventListener('click', (e) => {
    e.preventDefault();
    window.location.href = 'ktk-download://open';
  });
});
