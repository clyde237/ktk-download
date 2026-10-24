/**
 * ktk-download — Background Service Worker (Manifest V3)
 * Compatible avec Chrome, Edge, Firefox (109+) et Opera.
 */

const browserAPI = globalThis.browser ?? globalThis.chrome;

const KTK_SERVER_URL = 'http://127.0.0.1:42124';
const PING_ENDPOINT = `${KTK_SERVER_URL}/ping`;
const DOWNLOAD_ENDPOINT = `${KTK_SERVER_URL}/download`;

// Extensions et types MIME vidéo pris en charge pour l'interception réseau
const VIDEO_EXTENSIONS = [
  '.mp4',
  '.m3u8',
  '.mpd',
  '.webm',
  '.mkv',
  '.flv',
  '.ts',
  '.mov',
  '.avi',
  '.m4v'
];

const VIDEO_MIME_TYPES = [
  'video/mp4',
  'video/webm',
  'video/ogg',
  'video/quicktime',
  'video/x-matroska',
  'video/x-flv',
  'application/vnd.apple.mpegurl',
  'application/x-mpegurl',
  'application/dash+xml'
];

/**
 * Récupère la liste des médias associés à un onglet depuis storage.local.
 */
async function getTabMedia(tabId) {
  if (!tabId || tabId < 0) return [];
  const key = `media_tab_${tabId}`;
  const res = await browserAPI.storage.local.get(key);
  return res[key] || [];
}

/**
 * Enregistre la liste des médias d'un onglet et met à jour le badge.
 */
async function setTabMedia(tabId, mediaList) {
  if (!tabId || tabId < 0) return;
  const key = `media_tab_${tabId}`;
  await browserAPI.storage.local.set({ [key]: mediaList });
  await updateBadge(tabId, mediaList.length);
}

/**
 * Met à jour le badge d'action de l'extension.
 */
async function updateBadge(tabId, count) {
  if (!tabId || tabId < 0) return;
  try {
    const text = count > 0 ? (count > 99 ? '99+' : String(count)) : '';
    await browserAPI.action.setBadgeText({ text, tabId });
    if (count > 0) {
      await browserAPI.action.setBadgeBackgroundColor({
        color: '#2563eb',
        tabId
      });
    }
  } catch (_e) {
    // Les onglets non accessibles peuvent ignorer le badge
  }
}

/**
 * Détecte si une URL correspond à un flux ou fichier vidéo.
 */
function isVideoUrl(url) {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();
    return VIDEO_EXTENSIONS.some((ext) => path.endsWith(ext) || path.includes(ext));
  } catch {
    return false;
  }
}

/**
 * Détermine le label de format d'après l'URL ou le Content-Type.
 */
function resolveFormatLabel(url, mimeType) {
  const lowerUrl = url.toLowerCase();
  const lowerMime = (mimeType || '').toLowerCase();

  if (lowerUrl.includes('.m3u8') || lowerMime.includes('mpegurl')) return 'HLS (M3U8)';
  if (lowerUrl.includes('.mpd') || lowerMime.includes('dash+xml')) return 'DASH (MPD)';
  if (lowerUrl.includes('.webm') || lowerMime.includes('webm')) return 'WebM';
  if (lowerUrl.includes('.mp4') || lowerMime.includes('mp4')) return 'MP4';
  if (lowerUrl.includes('.mkv') || lowerMime.includes('matroska')) return 'MKV';
  if (lowerUrl.includes('.ts')) return 'TS Stream';
  return 'Vidéo';
}

/**
 * Déduit un titre lisible à partir de l'URL ou de la page.
 */
function extractTitleFromUrl(url) {
  try {
    const parsed = new URL(url);
    const pathSegments = parsed.pathname.split('/').filter(Boolean);
    const last = pathSegments[pathSegments.length - 1] || 'video';
    const decoded = decodeURIComponent(last).split('?')[0];
    return decoded.replace(/[_-]/g, ' ');
  } catch {
    return 'Vidéo détectée';
  }
}

/**
 * Interception réseau non bloquante des requêtes vidéo (webRequest).
 */
if (browserAPI.webRequest?.onHeadersReceived) {
  browserAPI.webRequest.onHeadersReceived.addListener(
    (details) => {
      // Ignorer les requêtes sans onglet ou requêtes internes
      if (!details.tabId || details.tabId < 0) return;

      const url = details.url;
      // Éviter d'intercepter les requêtes vers notre propre serveur local
      if (url.startsWith(KTK_SERVER_URL)) return;

      let mimeType = '';
      let contentLength = 0;

      if (details.responseHeaders) {
        for (const h of details.responseHeaders) {
          const name = h.name.toLowerCase();
          if (name === 'content-type') {
            mimeType = h.value || '';
          } else if (name === 'content-length') {
            contentLength = parseInt(h.value || '0', 10);
          }
        }
      }

      const isVideoMime = VIDEO_MIME_TYPES.some((t) => mimeType.toLowerCase().includes(t));
      const hasVideoExt = isVideoUrl(url);

      if (isVideoMime || hasVideoExt) {
        // Filtrer les minuscules requêtes d'images ou pings < 32 KB sauf manifests m3u8/mpd
        const isManifest = url.includes('.m3u8') || url.includes('.mpd');
        if (!isManifest && contentLength > 0 && contentLength < 32768) {
          return;
        }

        // Ajouter de manière asynchrone dans le stockage local
        void (async () => {
          const current = await getTabMedia(details.tabId);
          if (current.some((m) => m.url === url)) return;

          const format = resolveFormatLabel(url, mimeType);
          const title = extractTitleFromUrl(url);

          const newMedia = {
            id: `net_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
            url,
            title,
            format,
            mimeType,
            sizeBytes: contentLength > 0 ? contentLength : null,
            source: 'network',
            detectedAt: Date.now()
          };

          current.push(newMedia);
          await setTabMedia(details.tabId, current);

          // Notifier le content script du même onglet pour mettre à jour les overlays
          try {
            await browserAPI.tabs.sendMessage(details.tabId, {
              type: 'MEDIA_LIST_UPDATED',
              media: newMedia
            });
          } catch (_e) {
            // Content script pas encore injecté ou inactif
          }
        })();
      }
    },
    { urls: ['<all_urls>'] },
    ['responseHeaders']
  );
}

/**
 * Nettoie les médias d'un onglet lors du rechargement de page ou de navigation.
 */
browserAPI.tabs.onUpdated.addListener(async (tabId, changeInfo) => {
  if (changeInfo.status === 'loading') {
    await setTabMedia(tabId, []);
  }
});

/**
 * Nettoie le cache d'un onglet fermé.
 */
browserAPI.tabs.onRemoved.addListener(async (tabId) => {
  const key = `media_tab_${tabId}`;
  await browserAPI.storage.local.remove(key);
});

/**
 * Vérifie si l'application locale ktk-download est démarrée.
 */
async function checkDesktopAppStatus() {
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 1200);
    const res = await fetch(PING_ENDPOINT, {
      method: 'GET',
      signal: controller.signal
    });
    clearTimeout(timeoutId);
    if (!res.ok) return { connected: false };
    const data = await res.json();
    return { connected: true, data };
  } catch {
    return { connected: false };
  }
}

/**
 * Transmet un ordre de téléchargement au serveur local ktk-download.
 */
async function sendDownloadToApp(payload) {
  try {
    let resolvedUrl = payload.url;
    if (!resolvedUrl || resolvedUrl.startsWith('blob:') || resolvedUrl.startsWith('data:')) {
      const tabs = await browserAPI.tabs.query({ active: true, currentWindow: true });
      if (tabs && tabs[0]?.url && !tabs[0].url.startsWith('about:')) {
        resolvedUrl = tabs[0].url;
      }
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 4000);
    const res = await fetch(DOWNLOAD_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        url: resolvedUrl,
        title: payload.title || undefined,
        format_id: payload.formatId || undefined
      }),
      signal: controller.signal
    });
    clearTimeout(timeoutId);

    if (res.ok) {
      return { success: true };
    }
    const errText = await res.text();
    return { success: false, error: errText || 'Erreur réponse serveur' };
  } catch (err) {
    return {
      success: false,
      error:
        'ktk-download n’est pas joignable sur 127.0.0.1:42124. Vérifiez que l’application de bureau est ouverte.'
    };
  }
}

/**
 * Dispatcher central de messages de l'extension.
 */
browserAPI.runtime.onMessage.addListener((message, sender, sendResponse) => {
  const tabId = sender.tab?.id;

  if (message.type === 'REGISTER_MEDIA') {
    (async () => {
      const targetTabId = message.tabId || tabId;
      if (!targetTabId) {
        sendResponse({ success: false });
        return;
      }
      const current = await getTabMedia(targetTabId);
      const incoming = message.media;
      if (!incoming || !incoming.url) {
        sendResponse({ success: false });
        return;
      }

      if (!current.some((m) => m.url === incoming.url)) {
        current.push({
          id: incoming.id || `dom_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
          url: incoming.url,
          title: incoming.title || 'Vidéo de la page',
          format: incoming.format || 'Vidéo (DOM)',
          resolution: incoming.resolution || null,
          duration: incoming.duration || null,
          source: incoming.source || 'dom',
          detectedAt: Date.now()
        });
        await setTabMedia(targetTabId, current);
      }
      sendResponse({ success: true, count: current.length });
    })();
    return true; // Garde le canal ouvert pour sendResponse async
  }

  if (message.type === 'GET_TAB_MEDIA') {
    (async () => {
      const targetTabId = message.tabId || tabId;
      const mediaList = await getTabMedia(targetTabId);
      sendResponse({ media: mediaList });
    })();
    return true;
  }

  if (message.type === 'CHECK_APP_STATUS') {
    (async () => {
      const status = await checkDesktopAppStatus();
      sendResponse(status);
    })();
    return true;
  }

  if (message.type === 'TRIGGER_DOWNLOAD') {
    (async () => {
      const result = await sendDownloadToApp(message.payload);
      sendResponse(result);
    })();
    return true;
  }

  if (message.type === 'CLEAR_TAB_MEDIA') {
    (async () => {
      const targetTabId = message.tabId || tabId;
      if (targetTabId) {
        await setTabMedia(targetTabId, []);
      }
      sendResponse({ success: true });
    })();
    return true;
  }

  return false;
});
