/**
 * ktk-download — Content Script
 * Détection automatique des éléments vidéo (DOM) et bouton flottant de téléchargement (style IDM)
 * Isolé dans un Shadow DOM pour une imperméabilité totale aux styles du site hôte.
 */

(() => {
  const browserAPI = globalThis.browser ?? globalThis.chrome;

  // Empêcher l'injection multiple dans la même frame
  if (window.__ktk_download_injected__) return;
  window.__ktk_download_injected__ = true;

  // Cache des vidéos observées et de leurs boutons associés
  const trackedVideos = new Map();

  /**
   * Crée ou met à jour le conteneur Shadow DOM pour le bouton de téléchargement IDM.
   */
  function getOrCreateOverlay(video) {
    if (trackedVideos.has(video)) {
      return trackedVideos.get(video);
    }

    const host = document.createElement('div');
    host.className = 'ktk-video-overlay-host';
    host.style.position = 'absolute';
    host.style.top = '0';
    host.style.left = '0';
    host.style.zIndex = '2147483647'; // Valeur z-index maximale
    host.style.pointerEvents = 'none';

    // Shadow DOM fermé pour une isolation absolue
    const shadowRoot = host.attachShadow({ mode: 'closed' });

    // Styles modernes du bouton flottant (Thème clair strict)
    const style = document.createElement('style');
    style.textContent = `
      :host {
        all: initial;
        display: block;
      }
      .ktk-btn-wrap {
        position: absolute;
        pointer-events: auto;
        opacity: 0;
        transform: translateY(-4px);
        transition: opacity 0.2s ease, transform 0.2s ease;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
        box-sizing: border-box;
      }
      .ktk-btn-wrap.visible {
        opacity: 1;
        transform: translateY(0);
      }
      .ktk-download-btn {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        background: #ffffff;
        color: #1e293b;
        border: 1px solid #cbd5e1;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12), 0 1px 3px rgba(0, 0, 0, 0.08);
        padding: 5px 10px;
        border-radius: 6px;
        font-size: 12px;
        font-weight: 600;
        cursor: pointer;
        user-select: none;
        transition: all 0.15s ease;
      }
      .ktk-download-btn:hover {
        background: #2563eb;
        color: #ffffff;
        border-color: #1d4ed8;
        box-shadow: 0 6px 16px rgba(37, 99, 235, 0.35);
      }
      .ktk-download-btn:active {
        transform: scale(0.97);
      }
      .ktk-icon {
        width: 14px;
        height: 14px;
        fill: currentColor;
        flex-shrink: 0;
      }
      .ktk-badge {
        font-size: 10px;
        padding: 1px 4px;
        border-radius: 4px;
        background: #e2e8f0;
        color: #475569;
        font-weight: 700;
      }
      .ktk-download-btn:hover .ktk-badge {
        background: rgba(255, 255, 255, 0.25);
        color: #ffffff;
      }
      .ktk-toast {
        margin-top: 4px;
        padding: 6px 10px;
        background: #0f172a;
        color: #f8fafc;
        border-radius: 6px;
        font-size: 11px;
        line-height: 1.3;
        box-shadow: 0 4px 10px rgba(0, 0, 0, 0.2);
        display: none;
        max-width: 260px;
      }
      .ktk-toast.show {
        display: block;
      }
      .ktk-toast-error {
        background: #b91c1c;
      }
      .ktk-toast-success {
        background: #15803d;
      }
      .ktk-toast-action {
        display: inline-block;
        margin-top: 4px;
        color: #93c5fd;
        text-decoration: underline;
        cursor: pointer;
      }
    `;

    const wrap = document.createElement('div');
    wrap.className = 'ktk-btn-wrap';

    const btn = document.createElement('button');
    btn.className = 'ktk-download-btn';
    btn.type = 'button';
    btn.innerHTML = `
      <svg class="ktk-icon" viewBox="0 0 24 24">
        <path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM17 13l-5 5-5-5h3V9h4v4h3z"/>
      </svg>
      <span>Télécharger</span>
      <span class="ktk-badge">ktk</span>
    `;

    const toast = document.createElement('div');
    toast.className = 'ktk-toast';

    wrap.appendChild(btn);
    wrap.appendChild(toast);
    shadowRoot.appendChild(style);
    shadowRoot.appendChild(wrap);

    document.body.appendChild(host);

    const overlayData = {
      host,
      wrap,
      btn,
      toast,
      video,
      hideTimeout: null,
      updatePosition: () => positionOverlay(video, wrap)
    };

    // Gestion du clic pour lancer le téléchargement
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      e.preventDefault();
      await handleDownloadClick(video, btn, toast);
    });

    trackedVideos.set(video, overlayData);
    return overlayData;
  }

  /**
   * Positionne le bouton flottant précisément au coin supérieur droit de la vidéo.
   */
  function positionOverlay(video, wrap) {
    if (!video.isConnected) return;
    const rect = video.getBoundingClientRect();

    // Si la vidéo est invisible ou masquée
    if (rect.width < 120 || rect.height < 80) {
      wrap.classList.remove('visible');
      return;
    }

    const scrollX = window.scrollX || window.pageXOffset;
    const scrollY = window.scrollY || window.pageYOffset;

    // Décalage de 12px depuis le bord supérieur droit de la vidéo
    const top = rect.top + scrollY + 12;
    const left = rect.right + scrollX - wrap.offsetWidth - 12;

    wrap.style.top = `${Math.max(0, top)}px`;
    wrap.style.left = `${Math.max(0, left)}px`;
  }

  /**
   * Traitement du clic sur le bouton de téléchargement IDM.
   */
  async function handleDownloadClick(video, btn, toast) {
    const mediaUrl = video.currentSrc || video.src || getChildSource(video) || window.location.href;
    const pageTitle = document.title || 'Vidéo web';
    const resolution =
      video.videoWidth && video.videoHeight
        ? `${video.videoWidth}x${video.videoHeight}`
        : undefined;

    btn.disabled = true;
    const originalText = btn.innerHTML;
    btn.innerHTML = `<span>Envoi en cours...</span>`;

    try {
      const response = await browserAPI.runtime.sendMessage({
        type: 'TRIGGER_DOWNLOAD',
        payload: {
          url: mediaUrl,
          title: pageTitle,
          formatId: resolution ? `${video.videoHeight}p` : 'best'
        }
      });

      if (response && response.success) {
        showToast(toast, 'Transmis à ktk-download !', 'success');
      } else {
        const errorMsg = response?.error || 'ktk-download non connecté.';
        showToast(
          toast,
          `${errorMsg} <span class="ktk-toast-action" id="ktk-copy">Copier l'URL</span>`,
          'error',
          mediaUrl
        );
      }
    } catch (_err) {
      showToast(toast, 'Erreur de communication avec l’extension.', 'error');
    } finally {
      setTimeout(() => {
        btn.disabled = false;
        btn.innerHTML = originalText;
      }, 1500);
    }
  }

  /**
   * Affiche un message toast éphémère dans le Shadow DOM du bouton.
   */
  function showToast(toast, html, type, copyUrl) {
    toast.className = `ktk-toast show ktk-toast-${type}`;
    toast.innerHTML = html;

    if (copyUrl) {
      const copyBtn = toast.querySelector('#ktk-copy');
      if (copyBtn) {
        copyBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          navigator.clipboard.writeText(copyUrl);
          toast.textContent = 'Lien copié dans le presse-papier !';
        });
      }
    }

    setTimeout(() => {
      toast.className = 'ktk-toast';
    }, 4500);
  }

  /**
   * Trouve la première source d'un élément vidéo avec enfants <source>.
   */
  function getChildSource(video) {
    const sources = video.querySelectorAll('source');
    for (const s of sources) {
      if (s.src) return s.src;
    }
    return null;
  }

  /**
   * Enregistre la vidéo auprès du service worker de l'extension.
   */
  function registerVideo(video) {
    const src = video.currentSrc || video.src || getChildSource(video);
    if (!src) return;

    const resolution =
      video.videoWidth && video.videoHeight ? `${video.videoWidth}x${video.videoHeight}` : null;

    try {
      browserAPI.runtime.sendMessage({
        type: 'REGISTER_MEDIA',
        media: {
          url: src,
          title: document.title || 'Vidéo web',
          format: resolution ? `MP4 (${resolution})` : 'Vidéo (DOM)',
          resolution,
          duration: video.duration && !isNaN(video.duration) ? Math.round(video.duration) : null,
          source: 'dom'
        }
      });
    } catch (_e) {
      // Contexte d'extension parfois invalidé lors d'un rechargement
    }
  }

  /**
   * Configure les écouteurs sur une balise <video>.
   */
  function attachVideoListeners(video) {
    if (trackedVideos.has(video)) return;

    const overlay = getOrCreateOverlay(video);

    function showOverlay() {
      clearTimeout(overlay.hideTimeout);
      overlay.updatePosition();
      overlay.wrap.classList.add('visible');
    }

    function hideOverlay() {
      overlay.hideTimeout = setTimeout(() => {
        overlay.wrap.classList.remove('visible');
      }, 1200);
    }

    // Afficher au survol ou pendant la lecture
    video.addEventListener('mouseenter', showOverlay);
    video.addEventListener('mousemove', showOverlay);
    video.addEventListener('mouseleave', hideOverlay);
    video.addEventListener('play', () => {
      registerVideo(video);
      showOverlay();
      hideOverlay();
    });
    video.addEventListener('loadedmetadata', () => {
      registerVideo(video);
      overlay.updatePosition();
    });

    overlay.wrap.addEventListener('mouseenter', () => {
      clearTimeout(overlay.hideTimeout);
      overlay.wrap.classList.add('visible');
    });
    overlay.wrap.addEventListener('mouseleave', hideOverlay);

    // Enregistrer immédiatement si des métadonnées sont déjà chargées
    if (video.currentSrc || video.src) {
      registerVideo(video);
    }
  }

  /**
   * Scanne le document à la recherche d'éléments vidéo.
   */
  function scanVideos() {
    const videos = document.querySelectorAll('video');
    for (const v of videos) {
      attachVideoListeners(v);
    }
  }

  // Écouteur de repositionnement lors du scroll ou resize
  window.addEventListener(
    'scroll',
    () => {
      for (const [, overlay] of trackedVideos) {
        if (overlay.wrap.classList.contains('visible')) {
          overlay.updatePosition();
        }
      }
    },
    { passive: true }
  );

  window.addEventListener(
    'resize',
    () => {
      for (const [, overlay] of trackedVideos) {
        overlay.updatePosition();
      }
    },
    { passive: true }
  );

  // Observer de mutations DOM pour les applications monopages (YouTube, Twitter, Facebook, TikTok...)
  const observer = new MutationObserver((mutations) => {
    let shouldScan = false;
    for (const mutation of mutations) {
      if (mutation.addedNodes.length > 0) {
        shouldScan = true;
        break;
      }
    }
    if (shouldScan) {
      scanVideos();
    }
  });

  observer.observe(document.documentElement, {
    childList: true,
    subtree: true
  });

  // Premier scan dès l'exécution
  scanVideos();
})();
