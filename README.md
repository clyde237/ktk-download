# 🎬 ktk-download

> Application de bureau moderne, ultra-rapide et multi-plateforme pour le téléchargement de flux vidéo web (YouTube, flux HLS `.m3u8`, DASH, MP4 directs), combinée à une extension pour navigateur internet (façon IDM).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg?logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-v5_Runes-FF3E00.svg?logo=svelte&logoColor=white)](https://svelte.dev/)
[![CI Status](https://img.shields.io/badge/CI-Passing-brightgreen.svg)](<>)

---

## 📖 Sommaire

- [Vue d'ensemble & Vision](#-vue-densemble--vision)
- [Fonctionnalités clés](#-fonctionnalités-clés)
- [Architecture & Stack Technique](#-architecture--stack-technique)
- [Structure du Projet](#-structure-du-projet)
- [Prérequis Système Globaux](#-prérequis-système-globaux)
- [Procédure de Compilation par OS](#-procédure-de-compilation-par-os)
  - [1. Linux (.deb et .AppImage)](#1-linux-deb-et-appimage)
  - [2. Windows (.exe NSIS)](#2-windows-exe-nsis)
  - [3. macOS (.dmg)](#3-macos-dmg)
- [Procédure d'Installation des Binaires](#-procédure-dinstallation-des-binaires)
  - [Installation sous Linux](#installation-sous-linux)
  - [Installation sous Windows](#installation-sous-windows)
  - [Installation sous macOS](#installation-sous-macos)
- [Liaison avec l'Extension Navigateur (Style IDM)](#-liaison-avec-lextension-navigateur-style-idm)
  - [Principe du pont local sécurisé](#principe-du-pont-local-sécurisé)
  - [Installation dans Chrome, Brave & Chromium](#installation-dans-chrome-brave--chromium)
  - [Installation dans Microsoft Edge](#installation-dans-microsoft-edge)
  - [Installation dans Mozilla Firefox](#installation-dans-mozilla-firefox)
  - [Installation dans Opera](#installation-dans-opera)
- [Tests & Assurance Qualité](#-tests--assurance-qualité)
- [Documentation Détaillée](#-documentation-détaillée)
- [Roadmap du Projet](#-roadmap-du-projet)
- [Licence](#-licence)

---

## 🎯 Vue d'ensemble & Vision

**ktk-download** est une application desktop native conçue pour offrir une expérience utilisateur fluide, universelle et instantanée de téléchargement vidéo :

- **Support universel** : Détecte et extrait les flux vidéo YouTube, les lecteurs de streaming direct (HLS `.m3u8`, DASH `.mpd`), les fichiers statiques MP4/WebM et la plupart des plateformes multimédias prises en charge par `yt-dlp`.
- **Intégration transparente au navigateur** : Une extension web compatible Chrome, Edge, Firefox et Opera injecte un bouton « **Télécharger** » discret directement au-dessus des lecteurs vidéo et capture les flux réseau en tâche de fond.
- **File d'attente & Contrôle total** : Gestion multi-téléchargements, réordonnancement par priorité, mise en pause et reprise intelligente sans perte de progression.
- **Sélection de qualité sur-mesure** : Choix des résolutions disponibles (1080p, 720p, 480p) et extraction audio exclusive en MP3 haute fidélité.
- **Performance native & Sobriété** : Moteur écrit en Rust (Tauri v2) pour une empreinte mémoire minimale (moins de 40 Mo de RAM en fonctionnement) et des débits d'écriture maximisés.
- **Design épuré et moderne** : Interface claire, lumineuse, réactive et ergonomique conçue avec Svelte 5 et Tailwind CSS.

---

## ✨ Fonctionnalités clés

1. 🌐 **Extraction multi-sources** : Prise en charge native des URLs de plateformes complexes ou de liens de streaming direct.
2. 🚀 **Extension Navigateur Intégrée** : Bouton flottant façon _Internet Download Manager_ isolé dans un Shadow DOM fermé et renifleur réseau (sniffing de segments).
3. 📊 **Suivi en direct** : Barre de progression animée avec débit en temps réel, volume transféré et estimation du temps restant (ETA).
4. ⏸️ **Pause & Reprise résiliente** : Interruption sans corruption des fichiers partiels et reprise fluide.
5. 🗂️ **Gestionnaire de destination** : Sélecteur de dossier natif OS et accès direct d'un clic au fichier téléchargé dans l'explorateur de fichiers.
6. 📜 **Historique persistant** : Mémorisation des téléchargements passés avec re-téléchargement immédiat et nettoyage sélectif.
7. 🛡️ **Sécurité & Assainissement** : Validation stricte des protocoles (HTTP/HTTPS uniquement), protection SSRF contre les adresses locales non autorisées et élimination des traceurs publicitaires d'URL.

---

## 🛠️ Architecture & Stack Technique

Le projet repose sur une frontière étanche entre l'interface utilisateur et le moteur natif :

```text
ktk-download/
├── Frontend (SvelteKit 2 + Svelte 5 + Tailwind CSS)
│    ├── UI Réactive & Thème clair moderne (Runes $state, $derived)
│    ├── File d'attente (Download Queue) & Gestion des priorités
│    ├── Toasts de notification & Historique local
│    └── Communication asynchrone via Tauri IPC
│
├── Backend (Tauri v2 + Rust 2024 Edition)
│    ├── Commandes IPC (`download_video`, `inspect_url`, `cancel_download`, etc.)
│    ├── Serveur local sécurisé loopback HTTP 127.0.0.1:42124 pour l'extension
│    ├── Registre de téléchargements avec gestion des tokens d'annulation/pause
│    ├── Moteur d'extraction vidéo yt-dlp & streaming reqwest
│    └── Intégration système (boîtes de dialogue natives, ouverture de dossiers)
│
└── Extension Web (Manifest V3 Multi-Navigateurs)
     ├── Content Script : Bouton flottant IDM encapsulé dans un Shadow DOM
     ├── Service Worker : Sniffing réseau des requêtes vidéo et flux m3u8
     └── Popup UI : Vue d'ensemble des flux détectés sur la page active
```

---

## 📂 Structure du Projet

```text
ktk-download/
├── docs/                         # Guides approfondis (installation, compilation, etc.)
│   └── INSTALLATION.md           # Guide complet par système d'exploitation
├── extension/                    # Extension de navigateur Web (Manifest V3)
│   ├── manifest.json             # Manifeste multi-navigateurs
│   ├── background.js             # Service worker de sniffing réseau
│   ├── content.js                # Bouton flottant au survol des lecteurs vidéo
│   └── popup/                    # Interface de contrôle de l'extension
├── src/                          # Frontend SvelteKit / Svelte 5
│   ├── lib/
│   │   ├── components/           # Composants UI (UrlInput, Progress, History, etc.)
│   │   ├── stores/               # Stores réactifs Svelte 5 (downloads, history, toasts)
│   │   └── utils/                # Utilitaires de formatage, chemin et système
│   └── routes/                   # Pages et layouts de l'application
├── src-tauri/                    # Backend natif Rust (Tauri v2)
│   ├── src/
│   │   ├── commands.rs           # Commandes IPC exposées au frontend
│   │   ├── downloader.rs         # Moteur de téléchargement asynchrone & streaming
│   │   ├── extractor.rs          # Interfaçage avec yt-dlp et analyse des métadonnées
│   │   ├── models.rs             # Structures de données partagées
│   │   ├── quality.rs            # Analyse et sélection des formats/résolutions
│   │   ├── server.rs             # Serveur HTTP local dédié à l'extension (port 42124)
│   │   └── validator.rs          # Validation et assainissement strict des URLs
│   ├── tests/                    # Tests d'intégration IPC Tauri
│   ├── Cargo.toml                # Dépendances système Rust
│   └── tauri.conf.json           # Configuration globale Tauri (cibles, bundles, etc.)
└── package.json                  # Dépendances Node.js et scripts npm
```

---

## 📋 Prérequis Système Globaux

### 1. Outils de développement

- **Node.js** : v18 ou v20+ (LTS recommandé)
- **Rust & Cargo** : version stable (v1.78+) via [rustup.rs](https://rustup.rs/)

### 2. Outils multimédias (Recommandés)

- **ffmpeg** : Requis pour fusionner la vidéo et l'audio séparés (flux 1080p/4K) et convertir en MP3.
  - Linux : `sudo apt install ffmpeg`
  - macOS : `brew install ffmpeg`
  - Windows : `winget install Gyan.FFmpeg`
- **yt-dlp** : Moteur d'extraction des métadonnées vidéo.
  - Recommandé : `pip install --user "yt-dlp[default]"` (installe également le solveur JavaScript EJS pour YouTube).

---

## 🔨 Procédure de Compilation par OS

Pour cloner et préparer l'environnement :

```bash
git clone https://github.com/clyde237/ktk-download.git
cd ktk-download
npm install
```

### 1. Linux (.deb et .AppImage)

Installez les bibliothèques système WebKitGTK requises :

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Pour lancer en développement :

```bash
npm run tauri dev
```

Pour compiler les paquets de production optimisés :

```bash
# Générer le paquet Debian (.deb)
npm run tauri build -- --bundles deb

# Générer l'AppImage autonome (.AppImage)
npm run tauri build -- --bundles appimage
```

_Les artefacts générés sont disponibles dans `src-tauri/target/release/bundle/deb/` et `.../bundle/appimage/`._

---

### 2. Windows (.exe NSIS)

1. Installez les **Visual Studio C++ Build Tools** avec la charge de travail _Développement Desktop en C++_.
2. Exécutez la compilation :
   ```powershell
   npm run tauri build -- --bundles nsis
   ```

_L'installateur NSIS est généré dans `src-tauri/target/release/bundle/nsis/ktk-download_0.1.0_x64-setup.exe`._

---

### 3. macOS (.dmg)

1. Installez les outils de ligne de commande : `xcode-select --install`.
2. Lancez la création de l'image disque :
   ```bash
   npm run tauri build -- --bundles dmg
   ```

_L'image `.dmg` est générée dans `src-tauri/target/release/bundle/dmg/ktk-download_0.1.0_aarch64.dmg`._

---

## 📦 Procédure d'Installation des Binaires

### Installation sous Linux

- **Paquet Debian (.deb)** :
  ```bash
  sudo apt install ./ktk-download_0.1.0_amd64.deb
  ```
- **AppImage autonome** :
  ```bash
  chmod +x ktk-download_0.1.0_amd64.AppImage
  ./ktk-download_0.1.0_amd64.AppImage
  ```

### Installation sous Windows

- Double-cliquez sur `ktk-download_0.1.0_x64-setup.exe`.
- L'installation s'effectue dans l'espace utilisateur local (aucun privilège administrateur forcé).
- Les raccourcis sont automatiquement créés dans le menu Démarrer et sur le bureau.

### Installation sous macOS

- Ouvrez le fichier `ktk-download_0.1.0_aarch64.dmg`.
- Faites glisser l'icône **ktk-download** dans le dossier **Applications**.
- _Note Gatekeeper_ : En cas de message de développeur non vérifié, exécutez `xattr -cr /Applications/ktk-download.app` ou validez via _Préférences Système > Confidentialité et sécurité_.

---

## 🔌 Liaison avec l'Extension Navigateur (Style IDM)

L'extension permet de capturer les vidéos au survol d'un lecteur ou via l'analyse des flux réseau, exactement comme **Internet Download Manager (IDM)**.

```text
Navigateur (Extension) ──[HTTP POST 127.0.0.1:42124/download]──> Application Desktop ktk-download
```

### Principe du pont local sécurisé

- L'application ktk-download démarre automatiquement un micro-serveur HTTP local sur l'adresse de bouclage **`127.0.0.1:42124`**.
- L'accès est strictement réservé à la machine locale (aucun accès réseau distant n'est exposé).
- Des en-têtes CORS permissifs permettent à l'extension d'émettre des requêtes `OPTIONS`, `GET /ping` et `POST /download`.

---

### Installation dans Chrome, Brave & Chromium

1. Ouvrez `chrome://extensions/` dans votre barre d'adresse.
2. Activez le **Mode développeur** (en haut à droite).
3. Cliquez sur **Charger l'extension non empaquetée**.
4. Sélectionnez le dossier `extension/` du projet.

### Installation dans Microsoft Edge

1. Ouvrez `edge://extensions/`.
2. Activez le **Mode développeur** dans la colonne de gauche.
3. Cliquez sur **Charger les composants décompressés** et sélectionnez le dossier `extension/`.

### Installation dans Mozilla Firefox

1. Ouvrez `about:debugging#/runtime/this-firefox`.
2. Cliquez sur **Charger un module temporaire...**.
3. Choisissez le fichier `extension/manifest.json`.

### Installation dans Opera

1. Ouvrez `opera://extensions/`.
2. Activez le **Mode développeur**.
3. Cliquez sur **Charger l'extension décompressée** et sélectionnez le dossier `extension/`.

---

## 🧪 Tests & Assurance Qualité

Le projet intègre une suite de tests automatisés couvrant tous les niveaux de l'architecture :

```bash
# 1. Tests unitaires frontend (stores réactifs Svelte 5, formatage, gestion d'erreurs)
npm test

# 2. Validation TypeScript et vérification de conformité Svelte 5
npm run check

# 3. Linter et formatage du code
npm run lint

# 4. Tests unitaires et d'intégration IPC complets Rust
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## 📚 Documentation Détaillée

Pour des guides plus exhaustifs et des détails de configuration avancés :

- 📘 [Guide détaillé d'installation et de compilation par plateforme](docs/INSTALLATION.md)
- 🔌 [Manuel complet de l'extension de navigateur](extension/README.md)
- 🗺️ [Suivi d'avancement du projet (ROADMAP)](ROADMAP.md)

---

## 🗺️ Roadmap du Projet

- [x] Initialisation du squelette Tauri v2 avec SvelteKit & Svelte 5
- [x] Moteur de téléchargement Rust multi-sources (YouTube, flux HLS `.m3u8`, DASH, MP4 direct)
- [x] Interface utilisateur moderne (thème clair élégant, responsive, typographie Martian Mono)
- [x] Composant réactif de progression en temps réel (vitesse, pourcentage, ETA, bytes)
- [x] Sélecteur de qualité (1080p, 720p, 480p, conversion audio MP3)
- [x] Historique local des téléchargements avec ouverture directe du dossier
- [x] File d'attente multi-téléchargements avec priorisation, pause et reprise résiliente
- [x] Extension navigateur universelle (Chrome, Edge, Firefox, Opera) avec détection flottante IDM
- [x] Tests unitaires & intégration IPC complets (Rust mocké + Vitest)
- [x] Configuration des pipelines de build multi-plateformes (.deb, .AppImage, .exe, .dmg)
- [x] Documentation complète utilisateur et contributeur avec procédure par OS

---

## 📄 Licence

Ce projet est distribué sous licence libre [MIT](LICENSE).
Copyright © 2026 Kore-Teck. Tous droits réservés.
