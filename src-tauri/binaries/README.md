# External Binaries Directory

Ce répertoire accueille les binaires autonomes nécessaires au fonctionnement de l'application sous Windows :

- `yt-dlp.exe` : extracteur et téléchargeur de flux multimédias.
- `ffmpeg.exe` & `ffprobe.exe` : moteur de multiplexage audio/vidéo et conversion.

## Préparation automatique pour le packaging Windows

Pour télécharger et placer automatiquement les dernières versions de ces exécutables avant de packager l'installateur Windows (NSIS / MSI) :

```bash
npm run prepare:binaries
```

Ce script télécharge les exécutables officiels et les installe dans ce dossier.

> **Note CI / Linux / macOS** : Ce fichier `README.md` est conservé sous contrôle de version pour garantir que la ressource déclarée `binaries/*` dans `tauri.conf.json` résout toujours au moins un fichier valide lors des vérifications Cargo (`clippy`, `test`, `build`) sur tous les systèmes d'exploitation.
