# 🔌 Extension ktk-download — Module Navigateur Multi-Plateforme

Cette extension pour navigateur web intègre **ktk-download** directement dans vos pages internet préférées, répliquant l'expérience fluide et instantanée d'Internet Download Manager (IDM).

Conçue selon les standards modernes du **Manifest V3**, elle est universellement compatible avec :

- **Google Chrome**
- **Microsoft Edge**
- **Mozilla Firefox** (109+)
- **Opera & Brave**
- Tout navigateur basé sur **Chromium** (Vivaldi, Arc, etc.)

---

## 🌟 Fonctionnalités Clés

1. **Bouton flottant IDM sur les vidéos** :
   - Un bouton « **Télécharger** » discret et élégant apparaît automatiquement au survol ou à la lecture de n'importe quel lecteur vidéo (YouTube, streaming direct, réseaux sociaux, lecteurs HTML5).
   - Le composant est entièrement encapsulé dans un **Shadow DOM fermé**, empêchant tout conflit de style CSS ou d'interférences avec le site hôte.
2. **Détection réseau avancée (Sniffing)** :
   - Intercepte les flux directs et segments multimédias (`.mp4`, `.m3u8` HLS, `.mpd` DASH, `.webm`, `.ts`, etc.) via l'API réseau `webRequest`.
   - Affiche un compteur dynamique en temps réel sur le badge de l'icône dans la barre d'outils.
3. **Popup complet de gestion** :
   - Liste détaillée des vidéos et flux détectés sur l'onglet actif.
   - Indication de la qualité, du format, de la taille estimée et du titre du flux.
   - Bouton de téléchargement unitaire ou « **Tout télécharger** » en un clic.
   - Témoin visuel de connexion en temps réel avec l'application de bureau.
4. **Pont local sécurisé** :
   - Transmet instantanément le lien vidéo à l'application locale via l'API loopback sécurisée `http://127.0.0.1:42124/download`.

---

## ⚙️ Spécifications du Pont Local (Local Bridge)

L'application desktop ktk-download embarque un serveur HTTP asynchrone écrit en Rust (Tokio) écoutant exclusivement sur l'adresse de bouclage locale :

```text
Host: 127.0.0.1
Port: 42124
Sécurité: Aucun accès externe, communication inter-processus locale uniquement
```

### Endpoints exposés

#### 1. Vérification de connectivité : `GET /ping`

- **Méthode** : `GET` ou `OPTIONS` (pré-vol CORS)
- **Réponse HTTP 200** :
  ```json
  {
    "status": "ok",
    "app": "ktk-download",
    "version": "0.1.0"
  }
  ```
- L'extension effectue un ping périodique et affiche un voyant vert dans la popup lorsque l'application de bureau est prête à recevoir des flux.

#### 2. Envoi d'un téléchargement : `POST /download` (ou `POST /add`)

- **Corps de la requête (JSON)** :
  ```json
  {
    "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    "title": "Titre optionnel de la vidéo",
    "format_id": "1080p"
  }
  ```
- **Réponse HTTP 200** :
  ```json
  {
    "status": "queued",
    "message": "Téléchargement ajouté à la file d'attente de ktk-download"
  }
  ```
- À la réception du payload, l'application de bureau émet l'événement interne `extension-download` vers le frontend Tauri, qui l'enregistre et l'exécute automatiquement dans la file d'attente.

---

## 🚀 Guide d'Installation par Navigateur

### 1. Google Chrome, Brave & Chromium

1. Ouvrez l'URL interne dans votre navigateur :
   ```text
   chrome://extensions
   ```
2. Activez l'interrupteur **Mode développeur** (en haut à droite).
3. Cliquez sur le bouton **Charger l'extension non empaquetée** (_Load unpacked_).
4. Sélectionnez le dossier `extension/` du dépôt `ktk-download`.
5. Épinglez l'icône dans votre barre d'outils pour visualiser facilement les vidéos détectées.

### 2. Microsoft Edge

1. Rendez-vous sur :
   ```text
   edge://extensions
   ```
2. Activez le **Mode développeur** dans la colonne latérale gauche.
3. Cliquez sur **Charger les composants décompressés**.
4. Sélectionnez le dossier `extension/`.

### 3. Mozilla Firefox

1. Entrez dans la barre d'adresse :
   ```text
   about:debugging#/runtime/this-firefox
   ```
2. Cliquez sur le bouton **Charger un module temporaire...**.
3. Parcourez les fichiers et sélectionnez directement le fichier :
   ```text
   extension/manifest.json
   ```

### 4. Opera

1. Accédez à :
   ```text
   opera://extensions
   ```
2. Cochez l'option **Mode développeur** en haut à droite.
3. Cliquez sur **Charger l'extension décompressée** et ciblez le dossier `extension/`.

---

## 💡 Utilisation au Quotidien

1. **Démarrer l'application ktk-download** sur votre système.
2. Ouvrez votre navigateur et naviguez sur n'importe quel site hébergeant du contenu vidéo (YouTube, sites d'actualités, plateformes de replay, lecteurs HTML5).
3. **Deux modes de téléchargement au choix** :
   - **Mode Direct (Bouton flottant)** : Survoulez le lecteur vidéo ou lancez la lecture, puis cliquez sur le bouton flottant **« Télécharger »** apparu en haut à droite.
   - **Mode Avancé (Popup)** : Cliquez sur l'icône de l'extension dans votre barre de navigateur pour visualiser tous les flux audio/vidéo reniflés, sélectionner la résolution désirée et lancer le téléchargement individuel ou groupé.
4. Le téléchargement est instantanément transmis à ktk-download avec progression en direct et notifications système !

---

## 🛠️ Dépannage & Diagnostic

- **Le témoin reste rouge « Déconnecté »** :
  Assurez-vous que l'application de bureau ktk-download est bien en cours d'exécution.
  Vous pouvez vérifier son écoute locale via un terminal :
  ```bash
  curl -i http://127.0.0.1:42124/ping
  ```
- **Le bouton flottant n'apparaît pas sur un lecteur spécifique** :
  Certains lecteurs vidéo utilisent des iframes cross-domain sécurisées. Dans ce cas, ouvrez simplement la popup de l'extension dans votre barre d'outils pour télécharger le flux intercepté par le renifleur réseau.

---

## 🔗 Liens Utiles

- 📘 [Guide complet d'installation et de compilation par système d'exploitation](../docs/INSTALLATION.md)
- 🏠 [Documentation principale du projet](../README.md)
