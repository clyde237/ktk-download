# Extension ktk-download — Module Navigateur Multi-Plateforme

Cette extension pour navigateur web intègre **ktk-download** directement dans vos pages internet préférées (style IDM / Internet Download Manager).

Elle est conçue avec **Manifest V3** et est compatible avec :

- **Google Chrome**
- **Microsoft Edge**
- **Mozilla Firefox** (109+)
- **Opera & Brave**

---

## Fonctionnalités Clés

1. **Bouton flottant IDM sur les vidéos** :
   - Un bouton « **Télécharger** » discret et élégant apparaît au survol de n'importe quelle vidéo sur la page (YouTube, streaming direct, réseaux sociaux, plateformes de presse, lecteurs HTML5).
   - Le bouton est entièrement isolé dans un **Shadow DOM fermé**, empêchant tout conflit de style avec le site internet hôte.
2. **Détection réseau avancée (Sniffing)** :
   - Intercepte les flux directs et segments de streaming (`.mp4`, `.m3u8` HLS, `.mpd` DASH, `.webm`, `.ts`, etc.) via `webRequest`.
   - Compteur en temps réel sur l'icône de l'extension (badge).
3. **Popup complet de gestion** :
   - Liste des vidéos et flux détectés sur l'onglet actif.
   - Détails sur le format, la résolution et la taille estimée.
   - Bouton de téléchargement unitaire ou « **Tout télécharger** » en un clic.
   - Témoin en direct de connexion avec l'application de bureau ktk-download.
4. **Pont local sécurisé** :
   - Transmet instantanément le flux à ktk-download via l'API loopback locale sécurisée `http://127.0.0.1:42124/download`.

---

## Guide d'Installation par Navigateur

### 1. Google Chrome, Brave & Chromium

1. Ouvrez `chrome://extensions` dans la barre d'adresse.
2. Activez le **Mode développeur** (interrupteur en haut à droite).
3. Cliquez sur **Charger l'extension non empaquetée**.
4. Sélectionnez le dossier `extension/` du projet ktk-download.

### 2. Microsoft Edge

1. Ouvrez `edge://extensions` dans la barre d'adresse.
2. Activez le **Mode développeur** dans la barre latérale gauche.
3. Cliquez sur **Charger les composants décompressés**.
4. Sélectionnez le dossier `extension/`.

### 3. Mozilla Firefox

1. Ouvrez `about:debugging#/runtime/this-firefox` dans la barre d'adresse.
2. Cliquez sur **Charger un module temporaire...**.
3. Sélectionnez le fichier `manifest.json` situé dans le dossier `extension/`.

### 4. Opera

1. Ouvrez `opera://extensions` dans la barre d'adresse.
2. Activez le **Mode développeur** (en haut à droite).
3. Cliquez sur **Charger l'extension décompressée** et sélectionnez le dossier `extension/`.

---

## Utilisation

1. Démarrez l'application **ktk-download** sur votre ordinateur.
2. Naviguez sur n'importe quelle page web contenant des vidéos.
3. Lancez la lecture de la vidéo ou survolez-la :
   - Cliquez directement sur le bouton flottant « **Télécharger** » affiché au coin de la vidéo.
   - OU cliquez sur l'icône **ktk-download** dans la barre d'outils du navigateur pour voir l'ensemble des flux détectés et choisir la qualité voulue.
4. Le téléchargement s'ajoute immédiatement à la file d'attente de **ktk-download** !
