# 📘 Guide Complet d'Installation, de Compilation & de Liaison — ktk-download

Ce guide détaille l'intégralité des procédures d'installation, de compilation depuis les sources sur chaque système d'exploitation (Linux, Windows, macOS), ainsi que la configuration pas-à-pas de la liaison avec l'extension de navigateur.

---

## 📑 Sommaire

1. [Matrice de compatibilité & Formats pris en charge](#1-matrice-de-compatibilité--formats-pris-en-charge)
2. [Outils auxiliaires indispensables (yt-dlp, ffmpeg, runtime JS)](#2-outils-auxiliaires-indispensables)
3. [Prérequis système par OS pour la compilation](#3-prérequis-système-par-os-pour-la-compilation)
   - [Linux (Ubuntu, Debian, Fedora, Arch)](#31-linux-ubuntu-debian-fedora-arch)
   - [Windows (10 & 11)](#32-windows-10--11)
   - [macOS (Apple Silicon & Intel)](#33-macos-apple-silicon--intel)
4. [Compilation depuis les sources](#4-compilation-depuis-les-sources)
   - [4.1 Cloner le dépôt et installer les dépendances Node](#41-cloner-le-dépôt-et-installer-les-dépendances-node)
   - [4.2 Exécution en mode développement](#42-exécution-en-mode-développement)
   - [4.3 Compilation pour la production par OS](#43-compilation-pour-la-production-par-os)
5. [Installation des paquets précompilés](#5-installation-des-paquets-précompilés)
   - [Linux (.deb et .AppImage)](#51-linux-paquets-deb-et-appimage)
   - [Windows (.exe NSIS et .msi)](#52-windows-installeur-exe-nsis)
   - [macOS (.dmg)](#53-macos-image-disque-dmg)
6. [Liaison avec l'extension de navigateur](#6-liaison-avec-lextension-de-navigateur)
   - [6.1 Architecture du pont local sécurisé](#61-architecture-du-pont-local-sécurisé)
   - [6.2 Installation dans Google Chrome, Brave & Vivaldi](#62-installation-dans-google-chrome-brave--vivaldi)
   - [6.3 Installation dans Microsoft Edge](#63-installation-dans-microsoft-edge)
   - [6.4 Installation dans Mozilla Firefox](#64-installation-dans-mozilla-firefox)
   - [6.5 Installation dans Opera](#65-installation-dans-opera)
   - [6.6 Vérification et diagnostic de la liaison](#66-vérification-et-diagnostic-de-la-liaison)
7. [Résolution des problèmes fréquents (FAQ)](#7-résolution-des-problèmes-fréquents-faq)
8. [Validation & Tests de conformité](#8-validation--tests-de-conformité)

---

## 1. Matrice de compatibilité & Formats pris en charge

**ktk-download** est conçu pour fonctionner nativement sur les trois principaux écosystèmes informatiques :

| Système d'exploitation           | Architectures matérielles           | Formats d'installation produits  | Statut             |
| :------------------------------- | :---------------------------------- | :------------------------------- | :----------------- |
| **Linux** (Debian/Ubuntu/Mint)   | x86_64, aarch64                     | Paquet Debian `.deb`             | ✅ Support complet |
| **Linux** (Toutes distributions) | x86_64, aarch64                     | Exécutable autonome `.AppImage`  | ✅ Support complet |
| **Windows** (10 & 11)            | x86_64, ARM64                       | Installateur NSIS `.exe`, `.msi` | ✅ Support complet |
| **macOS** (10.15 Catalina+)      | Apple Silicon (M1/M2/M3), Intel x64 | Image disque `.dmg`, `.app`      | ✅ Support complet |

---

## 2. Outils auxiliaires indispensables

Pour extraire et convertir les flux vidéo de haute qualité (notamment 1080p, 4K et conversion audio MP3), l'application s'appuie sur deux utilitaires majeurs :

### 2.1 ffmpeg (Recommandé pour les flux séparés et le MP3)

Sur la plupart des plateformes vidéo (YouTube en particulier), les pistes vidéo haute résolution et audio sont diffusées séparément pour économiser la bande passante. `ffmpeg` est utilisé en tâche de fond pour fusionner ces flux sans perte de qualité.

- **Linux (Debian/Ubuntu)** :
  ```bash
  sudo apt install -y ffmpeg
  ```
- **macOS (via Homebrew)** :
  ```bash
  brew install ffmpeg
  ```
- **Windows (via winget ou Chocolatey)** :
  ```powershell
  winget install Gyan.FFmpeg
  # ou via Chocolatey
  choco install ffmpeg
  ```

### 2.2 yt-dlp & solveur EJS pour YouTube

L'application intègre un moteur de recherche automatique pour localiser `yt-dlp` : elle vérifie d'abord la variable d'environnement `YTDLP_PATH`, puis le dossier utilisateur `~/.local/bin`, les chemins système standards (`/usr/local/bin`, `/usr/bin`) et enfin le `PATH` système.

Pour garantir l'extraction des signatures déchiffrées de YouTube, installez le paquet complet comprenant le solveur EJS :

- **Installation universelle via pip / pipx** :
  ```bash
  pip install --user "yt-dlp[default]"
  # Sur Ubuntu 23.04+ / Debian 12 (avec PEP 668) :
  pipx install "yt-dlp[default]"
  ```
- **macOS** :
  ```bash
  brew install yt-dlp
  ```
- **Windows** :
  ```powershell
  winget install yt-dlp.yt-dlp
  ```

> [!NOTE]
> Un runtime JavaScript (`node`, `deno` ou `bun`) est automatiquement détecté par ktk-download et transmis à `yt-dlp` pour résoudre les algorithmes de validation vidéo complexes.

---

## 3. Prérequis système par OS pour la compilation

Si vous souhaitez compiler ktk-download depuis son code source, assurez-vous d'avoir les chaînes d'outils suivantes :

### 3.1 Linux (Ubuntu, Debian, Fedora, Arch)

1. **Outils fondamentaux** :
   - **Node.js** v18 ou v20+ : `sudo apt install nodejs npm` ou via [nvm](https://github.com/nvm-sh/nvm).
   - **Rust & Cargo** (stable) :
     ```bash
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
     source "$HOME/.cargo/env"
     ```

2. **Bibliothèques de développement système (WebKitGTK, GTK3, AppIndicator)** :
   - **Debian / Ubuntu / Linux Mint** :
     ```bash
     sudo apt update
     sudo apt install -y \
       libwebkit2gtk-4.1-dev \
       build-essential \
       curl \
       wget \
       file \
       libxdo-dev \
       libssl-dev \
       libayatana-appindicator3-dev \
       librsvg2-dev
     ```
   - **Fedora / RHEL** :
     ```bash
     sudo dnf install -y \
       webkit2gtk4.1-devel \
       openssl-devel \
       curl \
       wget \
       libappindicator-gtk3-devel \
       librsvg2-devel
     ```
   - **Arch Linux / Manjaro** :
     ```bash
     sudo pacman -S --needed \
       webkit2gtk-4.1 \
       base-devel \
       curl \
       wget \
       openssl \
       libappindicator-gtk3 \
       librsvg
     ```

### 3.2 Windows (10 & 11)

1. **Visual Studio C++ Build Tools** :
   - Téléchargez et installez l'outil [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
   - Lors de l'installation, cochez le composant : **Développement Desktop en C++** (Desktop development with C++).
2. **WebView2 Runtime** :
   - Déjà inclus nativement sur Windows 10 et 11. S'il est absent, installez le [programme d'amorçage Evergreen WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
3. **Node.js & Rust** :
   - Installez Node.js LTS via [nodejs.org](https://nodejs.org/).
   - Installez Rustup via [rustup.rs](https://rustup.rs/) (sélectionnez l'architecture par défaut `x86_64-pc-windows-msvc`).

### 3.3 macOS (Apple Silicon & Intel)

1. **Outils de ligne de commande Apple Xcode** :
   ```bash
   xcode-select --install
   ```
2. **Node.js & Rust** :
   - Via Homebrew : `brew install node`
   - Rust via Rustup : `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

---

## 4. Compilation depuis les sources

### 4.1 Cloner le dépôt et installer les dépendances Node

```bash
git clone https://github.com/clyde237/ktk-download.git
cd ktk-download
npm install
```

### 4.2 Exécution en mode développement

Pour lancer l'application en mode interactif avec rechargement à chaud (Hot Module Replacement pour le frontend Svelte 5 et recompilation incrémentale Rust) :

```bash
npm run tauri dev
```

### 4.3 Compilation pour la production par OS

Pour générer des binaires hautement optimisés (optimisation `-C opt-level=3`, panic abort, strip des symboles de débogage et Link Time Optimization `LTO`) :

#### A. Compilation sous Linux

Génère le paquet Debian (`.deb`) et l'image autonome (`.AppImage`) :

```bash
# Générer toutes les cibles Linux configurées
npm run tauri build

# Ou cibler un format spécifique :
npm run tauri build -- --bundles deb
npm run tauri build -- --bundles appimage
```

Les fichiers générés se trouveront dans :

- `src-tauri/target/release/bundle/deb/ktk-download_<version>_<arch>.deb`
- `src-tauri/target/release/bundle/appimage/ktk-download_<version>_<arch>.AppImage`

#### B. Compilation sous Windows

Génère l'installateur NSIS exécutable (`.exe`) :

```powershell
npm run tauri build -- --bundles nsis
```

L'artefact produit se situe dans :

- `src-tauri/target/release/bundle/nsis/ktk-download_<version>_x64-setup.exe`

#### C. Compilation sous macOS

Génère le paquet d'application `.app` et le fichier d'installation disque `.dmg` :

```bash
npm run tauri build -- --bundles dmg
```

L'artefact produit se situe dans :

- `src-tauri/target/release/bundle/dmg/ktk-download_<version>_<arch>.dmg`

---

## 5. Installation des paquets précompilés

Si vous avez téléchargé les binaires officiels depuis la page [Releases GitHub](https://github.com/clyde237/ktk-download/releases) :

### 5.1 Linux (Paquets .deb et .AppImage)

#### Méthode 1 : Paquet Debian / Ubuntu (.deb)

Ouvrez un terminal dans le répertoire contenant le fichier téléchargé et exécutez :

```bash
sudo apt install ./ktk-download_0.1.0_amd64.deb
```

> [!TIP]
> L'utilisation de `apt install ./<fichier>.deb` au lieu de `dpkg -i` permet de résoudre et d'installer automatiquement toutes les dépendances requises (`libwebkit2gtk`, `libayatana-appindicator3`, etc.).

Pour désinstaller ultérieurement :

```bash
sudo apt remove ktk-download
```

#### Méthode 2 : Image autonome (.AppImage)

L'AppImage ne nécessite aucune installation dans les dossiers racine du système :

```bash
# Rendre le fichier exécutable
chmod +x ktk-download_0.1.0_amd64.AppImage

# Lancer l'application
./ktk-download_0.1.0_amd64.AppImage
```

### 5.2 Windows (Installeur .exe NSIS)

1. Double-cliquez sur `ktk-download_0.1.0_x64-setup.exe`.
2. L'installateur configure automatiquement l'application dans l'espace utilisateur local (`%LOCALAPPDATA%\Programs\ktk-download`) : aucune élévation de privilèges administrateur (UAC) n'est requise.
3. Un raccourci est automatiquement créé sur votre bureau et dans votre menu Démarrer.
4. L'application peut être désinstallée proprement à tout moment via **Paramètres Windows > Applications installées**.

### 5.3 macOS (Image disque .dmg)

1. Double-cliquez sur le fichier `ktk-download_0.1.0_aarch64.dmg` (ou `x64.dmg` selon votre processeur).
2. Faites glisser l'icône **ktk-download** vers le raccourci du dossier **Applications**.
3. Lancez l'application depuis le **Launchpad** ou le dossier **/Applications**.

> [!NOTE]
> **Gestion de Gatekeeper (macOS)** : Si macOS affiche le message d'avertissement _"ktk-download ne peut être ouvert car le développeur n'a pas pu être vérifié"_ (lors de l'utilisation d'un binaire communautaire sans certificat commercial Apple Developer payant) :
>
> - Ouvrez **Préférences Système > Confidentialité et sécurité** et cliquez sur **Ouvrir quand même**.
> - Ou exécutez la commande suivante dans le terminal :
>   ```bash
>   xattr -cr /Applications/ktk-download.app
>   ```

---

## 6. Liaison avec l'extension de navigateur

L'application **ktk-download** intègre une extension pour navigateurs modernes (Manifest V3) qui réplique l'expérience d'Internet Download Manager (IDM) : détection automatique des vidéos au survol, capture des flux réseau HLS/DASH/MP4 et envoi direct à l'application de bureau en un clic.

### 6.1 Architecture du pont local sécurisé

```text
┌───────────────────────────────────────────────┐
│              Navigateur Web                   │
│  (Chrome, Edge, Firefox, Opera, Brave)        │
│                                               │
│  • Content Script : Bouton flottant sur vidéo │
│  • Background SW : Sniffer de flux réseau     │
│  • Popup : Sélecteur de qualité & formats     │
└───────────────────────┬───────────────────────┘
                        │ Requête HTTP POST
                        │ (JSON Payload avec CORS)
                        ▼
┌───────────────────────────────────────────────┐
│        Serveur Loopback ktk-download          │
│            127.0.0.1:42124 (Rust)             │
│                                               │
│  • GET  /ping      -> Test de connectivité    │
│  • POST /download  -> Ingestion du flux       │
└───────────────────────┬───────────────────────┘
                        │ Événement IPC Tauri
                        ▼
┌───────────────────────────────────────────────┐
│       Moteur de téléchargement Desktop        │
│  File d'attente, progression & stockage local │
└───────────────────────────────────────────────┘
```

- **Sécurité stricte** : Le serveur d'écoute n'est lié qu'à l'adresse IP locale `127.0.0.1:42124`. Aucun ordinateur externe sur votre réseau local ne peut y accéder.
- **Autorisation CORS** : Les en-têtes HTTP retournent les autorisations nécessaires pour permettre aux scripts de l'extension de contacter l'application sans restriction de cross-origin.

---

### 6.2 Installation dans Google Chrome, Brave & Vivaldi

1. Ouvrez votre navigateur et accédez à :
   ```text
   chrome://extensions/
   ```
2. Activez l'interrupteur **Mode développeur** situé en haut à droite de l'écran.
3. Cliquez sur le bouton **Charger l'extension non empaquetée** (_Load unpacked_).
4. Sélectionnez le dossier `extension/` situé à la racine du dépôt `ktk-download`.
5. L'icône **ktk-download** apparaît immédiatement dans votre barre d'extensions. Nous vous recommandons de l'épingler pour un accès rapide.

---

### 6.3 Installation dans Microsoft Edge

1. Ouvrez Microsoft Edge et saisissez :
   ```text
   edge://extensions/
   ```
2. Dans le menu latéral gauche, activez l'interrupteur **Mode développeur**.
3. Cliquez sur le bouton **Charger les composants décompressés** en haut à droite.
4. Naviguez jusqu'au dossier `extension/` du projet et validez.

---

### 6.4 Installation dans Mozilla Firefox

1. Ouvrez Firefox et entrez l'adresse suivante :
   ```text
   about:debugging#/runtime/this-firefox
   ```
2. Cliquez sur le bouton **Charger un module temporaire...** (_Load Temporary Add-on..._).
3. Parcourez votre système de fichiers, entrez dans le dossier `extension/` et sélectionnez le fichier :
   ```text
   extension/manifest.json
   ```
4. L'extension est activée pour votre session Firefox.

---

### 6.5 Installation dans Opera

1. Ouvrez Opera et rendez-vous sur :
   ```text
   opera://extensions/
   ```
2. Cochez la case **Mode développeur** en haut à droite.
3. Cliquez sur **Charger l'extension décompressée** et sélectionnez le dossier `extension/`.

---

### 6.6 Vérification et diagnostic de la liaison

1. **Lancer l'application de bureau ktk-download**.
2. Cliquez sur l'icône de l'extension dans la barre d'outils de votre navigateur :
   - **Témoin vert « Connecté »** : L'extension a réussi la négociation `GET /ping` avec l'application de bureau.
   - **Témoin rouge « Déconnecté »** : L'application n'est pas ouverte ou le port `42124` est bloqué par une politique de sécurité locale.
3. Rendez-vous sur une page contenant une vidéo (par exemple YouTube ou un lecteur HTML5) :
   - Au survol du lecteur, un bouton flottant discret **Télécharger** s'affiche en haut à droite de la vidéo.
   - Cliquez dessus : la vidéo est immédiatement transmise à votre application ktk-download et le téléchargement commence !

---

## 7. Résolution des problèmes fréquents (FAQ)

### Q1 : L'extension indique « Déconnecté » alors que l'application est ouverte

- Vérifiez manuellement que le serveur écoute en ouvrant cette URL dans votre navigateur ou via un terminal :
  ```bash
  curl http://127.0.0.1:42124/ping
  ```
  Le serveur doit vous répondre : `{"status":"ok","app":"ktk-download","version":"0.1.0"}`.
- Si le port est déjà occupé par un autre processus local, libérez le port `42124` ou vérifiez vos règles de pare-feu applicatif local.

### Q2 : YouTube ne propose que des formats basse résolution ou une erreur « signature decryption »

- Assurez-vous d'avoir installé `yt-dlp` avec le solveur EJS :
  ```bash
  pip install --upgrade "yt-dlp[default]"
  ```
- Vérifiez la présence de `node` ou `deno` dans votre `PATH` pour que yt-dlp puisse exécuter les scripts de signature.

### Q3 : Erreur « No such file or directory » sur ffmpeg lors de la fusion 1080p

- Installez `ffmpeg` via votre gestionnaire de paquets (`apt install ffmpeg`, `brew install ffmpeg` ou `winget install Gyan.FFmpeg`).
- Redémarrez l'application ktk-download pour qu'elle prenne en compte le nouveau binaire.

---

## 8. Validation & Tests de conformité

Avant toute contribution ou déploiement, vous pouvez exécuter la suite de tests automatisés :

```bash
# 1. Tests unitaires frontend (49 tests Vitest)
npm test

# 2. Vérification des types Svelte & TypeScript (svelte-check)
npm run check

# 3. Linter & respect du style de code
npm run lint

# 4. Tests unitaires et intégration backend Rust (68 tests cargo)
cargo test --manifest-path src-tauri/Cargo.toml
```

Toutes les suites doivent s'exécuter avec **zéro erreur** pour garantir la fiabilité de production.
