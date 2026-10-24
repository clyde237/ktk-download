# Roadmap — ktk-download
> ktk-download is a modern, cross-platform desktop application for downloading videos from the web, built with Tauri v2 and Rust backend, and SvelteKit 2 + Svelte 5 frontend. It aims to provide universal video extraction (YouTube, HLS/MP4 streams), real-time progress tracking, and native performance with minimal memory footprint.

## Phase 1 : Socle & Configuration
- [x] Initialiser le projet Tauri v2 avec SvelteKit 2 et Svelte 5, configurer TypeScript en mode strict et les dépendances Node.js
- [x] Configurer la structure Rust backend dans src-tauri/lib.rs avec les commandes IPC download_video et inspect_url, en définissant les types d'erreur dédiés
- [x] Installer et vérifier les dépendances système requises pour Tauri sur Linux, macOS et Windows (webkit2gtk, build-essential, WebView2, etc.)
- [x] Mettre en place le linting, le formatage (Rustfmt, Prettier) et un pipeline CI GitHub Actions basique pour la validation à chaque push
- [x] S'assurer que le projet compile en mode développement avec `npm run tauri dev` et que l'interface Svelte s'affiche correctement
- [x] Initialiser un depot local git; et connecter au depot distant : git remote add origin https://github.com/clyde237/ktk-download.git; pousse sur deux branches main et develop

## Phase 2 : Backend & Moteur de Téléchargement
- [x] Implémenter la commande IPC download_video en Rust avec support streaming asynchrone et émission d'événements de progression via l'API Tauri IPC
- [x] Intégrer yt-dlp (ou des extracteurs bundlés) comme moteur d'extraction vidéo sous-jacent avec support YouTube et flux web génériques
- [x] Développer la logique de validation et d'assainissement des URLs avec vérification stricte du protocole et des domaines autorisés
- [x] Ajouter une gestion d'erreurs robuste : timeouts réseau, URLs invalides, sites non supportés, et sérialisation d'erreurs utilisateur-friendly

## Phase 3 : Interface Utilisateur & Expérience Utilisateur
- [x] Créer le composant Svelte 5 d'entrée d'URL avec validation en temps réel, détection de service et support copier-coller
- [x] Implémenter la barre de progression réactive liée au store `$state` Svelte 5, mise à jour asynchrone via les événements IPC Tauri
- [x] Concevoir et intégrer le thème clair moderne avec échelle typographique, palette de couleurs et mise en page réactive (Tailwind CSS ou CSS modules)
- [x] Développer le sélecteur de dossier de destination avec API file picker native et accès rapide aux dossiers récemment utilisés
- [x] Ajouter un système de notifications (toasts) pour le démarrage, la complétion, les erreurs et les jalons de progression du téléchargement

## Phase 4 : Fonctionnalités Avancées & Raffinement
- [x] Implémenter la sélection de qualité (1080p, 720p, 480p, MP3 audio) avec liaison d'état frontend et passage au backend
- [x] Construire le magasin d'historique des téléchargements (localStorage ou SQLite via plugin Tauri) avec ouverture directe du dossier et ré-exécution
- [x] Développer le système de file d'attente de téléchargements multiples avec réordonnancement par priorité, pause et reprise via des tokens de statut
- [x] Intégrer l'API d'accès au système de fichiers pour ouvrir directement le dossier contenant les fichiers téléchargés
- [x] Peaufiner les états de chargement, les bordures d'erreur et les états vides sur l'ensemble des composants
- [x] creer une extension compatible (firefox, edge, chorme et opera); une fois l'extension installé elle detectera les videos sur n'importe quel page et proposera de la telechargé (comme le fait IDM).

## Phase 5 : Tests, Déploiement & Release
- [ ] Écrire des tests unitaires pour la logique Rust de téléchargement (interactions yt-dlp mockées) et des tests unitaires Svelte avec Vitest
- [ ] Mettre en place des tests d'intégration pour les round-trips IPC Tauri (URL → téléchargement → progression → achèvement)
- [ ] Configurer les pipelines de build Tauri pour toutes les cibles (.deb, .AppImage, .exe, .dmg) avec mise en place de code signing
- [ ] Générer les assets de release et automatiser le bumping de version via `bumpp` ou équivalent
- [ ] Rédiger la documentation utilisateur et contributeur, et valider les prérequis spécifiques à chaque plateforme pour la distribution
