# 🎬 ktk-download

> Application de bureau moderne, ultra-rapide et multi-plateforme pour le téléchargement de vidéos depuis le web (YouTube, plateformes de streaming telles que MoviBox, etc.).

---

## 📖 Sommaire

- [Vue d'ensemble & Vision](#-vue-densemble--vision)
- [Historique & Évolution](#-historique--évolution)
- [Fonctionnalités clés](#-fonctionnalités-clés)
- [Architecture & Stack Technique](#-architecture--stack-technique)
- [Structure du Projet](#-structure-du-projet)
- [Prérequis Système](#-prérequis-système)
- [Installation & Lancement](#-installation--lancement)
  - [1. Cloner le projet](#1-cloner-le-projet)
  - [2. Installer les dépendances](#2-installer-les-dépendances)
  - [3. Démarrer en mode développement](#3-démarrer-en-mode-développement)
  - [4. Compiler pour la production](#4-compiler-pour-la-production)
- [Roadmap & Prochaines étapes](#-roadmap--prochaines-étapes)
- [Bonnes pratiques & Contribution](#-bonnes-pratiques--contribution)
- [Licence](#-licence)

---

## 🎯 Vue d'ensemble & Vision

**ktk-download** est une application desktop native conçue pour offrir une expérience utilisateur fluide, rapide et universelle de récupération de contenus vidéo en ligne.

L'objectif principal est de s'affranchir des contraintes des scripts traditionnels mono-plateformes :

- **Prise en charge universelle** : Au-delà de YouTube, supporter la majorité des sites hébergeant du contenu vidéo (sites de streaming direct comme [MoviBox](https://movibox.net/), lecteurs HLS/m3u8, MP4 directs, plateformes communautaires).
- **Interface graphique moderne** : Une interface soignée, claire et épurée (axée sur un thème clair moderne pour le moment, privilégiant la lisibilité et l'ergonomie).
- **Suivi en temps réel** : Barre de progression dynamique avec vitesse de transfert, pourcentage précis, volume téléchargé et estimation du temps restant.
- **Performance native & Légèreté** : Consommation mémoire minimale et vitesse d'exécution optimale grâce au runtime Rust de Tauri v2.

---

## 🔄 Historique & Évolution

Ce projet est la réécriture complète et la modernisation d'un premier outil en ligne de commande :

| Critères                | Version Initiale (Python)                         | Version Actuelle (`ktk-download`)                                      |
| :---------------------- | :------------------------------------------------ | :--------------------------------------------------------------------- |
| **Interface**           | Terminal / CLI rudimentaire                       | Interface Desktop moderne (SvelteKit + Tauri v2)                       |
| **Technologies**        | Python 3 + `pytube`                               | Rust (Backend natif) + SvelteKit / Svelte 5 (Frontend)                 |
| **Couverture de sites** | YouTube uniquement                                | Multi-sites (YouTube, [MoviBox](https://movibox.net/), flux web, etc.) |
| **Contrôle & Qualité**  | Basse résolution fixe (`get_lowest_resolution()`) | Choix de résolutions, extraction audio/vidéo                           |
| **Retour utilisateur**  | Messages console statiques                        | Barre de progression en temps réel & notifications                     |
| **Portabilité**         | Dépendance à un interpréteur Python local         | Binaire natif autonome (.deb, .AppImage, .exe, .dmg)                   |

---

## ✨ Fonctionnalités clés

- 🌐 **Extraction multi-sources** : Détection automatique des flux vidéo sur YouTube, MoviBox et autres plateformes de diffusion.
- 📊 **Progression dynamique** : Barre d'avancement animée et réactive alimentée par les événements IPC Tauri.
- ⚡ **Performances Rust** : Téléchargement asynchrone non bloquant avec streaming des paquets sur le disque.
- 🎨 **Design épuré et moderne** : Interface intuitive, aérée et réactive conçue avec Svelte 5 et Tailwind/CSS moderne.
- 📁 **Gestionnaire de destination** : Sélection intuitive du dossier de téléchargement avec accès rapide au fichier téléchargé.
- 🛡️ **Gestion robuste des erreurs** : Validation stricte des URLs, gestion des interruptions réseau et timeouts explicites.

---

## 🛠️ Architecture & Stack Technique

L'application repose sur le découpage strict des responsabilités entre l'interface utilisateur et le moteur système :

```text
ktk-download/
├── Frontend (SvelteKit 2 + Svelte 5 + Vite)
│    ├── UI / Composants modernes (Thème clair épuré)
│    ├── Saisie & validation de l'URL
│    ├── Suivi de la barre de progression (Store / Runes $state)
│    └── Communication asynchrone via Tauri IPC
│
└── Backend (Tauri v2 + Rust)
     ├── Commandes IPC (`download_video`, `inspect_url`, etc.)
     ├── Moteur de scraping & extraction de flux (HLS/DASH/MP4/yt-dlp)
     ├── Streaming asynchrone & émission d'événements de progression
     └── Gestion du système de fichiers et des métadonnées
```

### Technologies utilisées

- **Framework Desktop** : [Tauri v2](https://tauri.app/) (légèreté, sécurité, performances natives)
- **Backend** : [Rust](https://www.rust-lang.org/) (gestion des threads, flux réseau, fichiers)
- **Frontend** : [SvelteKit 2](https://kit.svelte.dev/) avec [Svelte 5](https://svelte.dev/) (réactivité avec les _Runes_ `$state`, `$derived`)
- **Outillage JS/TS** : [Vite](https://vite.dev/), [TypeScript](https://www.typescriptlang.org/)

---

## 📂 Structure du Projet

```text
ktk-download/
├── src/                          # Code source Frontend
│   ├── routes/
│   │   ├── +page.svelte          # Page principale de téléchargement
│   │   └── +layout.ts            # Configuration du layout SvelteKit
│   └── app.html                  # Template HTML de base
├── src-tauri/                    # Code source Backend (Rust)
│   ├── src/
│   │   ├── main.rs               # Point d'entrée de l'exécutable
│   │   └── lib.rs                # Logique des commandes et événements Tauri
│   ├── Cargo.toml                # Dépendances Rust
│   └── tauri.conf.json           # Configuration globale Tauri v2
├── static/                       # Ressources statiques
├── package.json                  # Scripts et dépendances Node.js
├── svelte.config.js              # Configuration de Svelte
├── tsconfig.json                 # Configuration TypeScript
├── vite.config.js                # Configuration du bundler Vite
└── README.md                     # Documentation du projet
```

---

## 📋 Prérequis Système

Avant de commencer, assurez-vous d'avoir installé sur votre machine :

### 1. Outils de base

- **Node.js** : version LTS recommandée (v18+)
- **Rust & Cargo** : version stable (v1.78+) via [rustup.rs](https://rustup.rs/)

### 2. Dépendances système pour Tauri

- **Sur Linux (Debian / Ubuntu)** :
  ```bash
  sudo apt update
  sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```
- **Sur macOS** :
  - Xcode Command Line Tools : `xcode-select --install`
- **Sur Windows** :
  - Microsoft C++ Build Tools et WebView2 (inclus par défaut sur Windows 10/11)

---

## 🚀 Installation & Lancement

### 1. Cloner le projet

```bash
git clone https://github.com/clyde237/ktk-download.git
cd ktk-download
```

### 2. Installer les dépendances

```bash
npm install
```

### 3. Démarrer en mode développement

Lance le serveur de développement Vite et la fenêtre native Tauri :

```bash
npm run tauri dev
```

### 4. Compiler pour la production

Pour générer l'exécutable natif et les installateurs prêts pour la distribution :

```bash
npm run tauri build
```

Les exécutables générés se trouvent dans le répertoire `src-tauri/target/release/bundle/`.

---

## 🗺️ Roadmap & Prochaines étapes

- [x] Initialisation du squelette Tauri v2 avec SvelteKit & Svelte 5
- [ ] Conception de l'interface utilisateur moderne (thème clair élégant, typographie travaillée)
- [ ] Implémentation du composant de barre de progression en temps réel
- [ ] Moteur de téléchargement Rust multi-sources :
  - [ ] Intégration / support des extracteurs universels (YouTube, flux HLS `.m3u8`, MP4 direct)
  - [ ] Support ciblé des sites de streaming vidéo tels que [MoviBox](https://movibox.net/)
- [ ] Sélecteur de qualité (1080p, 720p, 480p, Audio MP3)
- [ ] Historique local des téléchargements avec ouverture directe du dossier
- [ ] File d'attente de téléchargements simultanés (téléchargements multiples)
- [ ] Option de pause / reprise des téléchargements volumineux

---

## 🤝 Bonnes pratiques & Contribution

Le projet respecte les principes fondamentaux de qualité logicielle :

- **Séparation claire des responsabilités** : L'interface se concentre sur l'interaction utilisateur ; le backend Rust garantit l'intégrité, la robustesse et la rapidité des opérations d'E/S.
- **Validation stricte** : Chaque URL saisie est inspectée et assainie avant tout traitement.
- **Gestion explicite des erreurs** : Messages clairs et non intrusifs en cas de rupture de lien ou de problème réseau.

Les contributions, suggestions et signalements d'anomalies sont les bienvenus via les [Issues GitHub](https://github.com/clyde237/ktk-download/issues).

---

## 📄 Licence

Ce projet est distribué sous licence [MIT](LICENSE).
