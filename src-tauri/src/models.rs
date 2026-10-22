use serde::{Deserialize, Serialize};

/// Représente un format ou une qualité disponible pour une vidéo inspectée.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoFormat {
    /// Identifiant unique du format (ex: "1080p", "720p", "best", "audio_only")
    pub format_id: String,
    /// Libellé lisible pour l'interface utilisateur (ex: "1080p (Full HD)", "720p (HD)", "Audio seul (MP3)")
    pub label: String,
    /// Résolution d'affichage si applicable (ex: "1920x1080")
    pub resolution: Option<String>,
    /// Extension du fichier de sortie (ex: "mp4", "webm", "mp3")
    pub extension: String,
    /// Taille estimée en octets si disponible
    pub file_size_bytes: Option<u64>,
    /// Indique si ce format inclut le flux vidéo
    pub has_video: bool,
    /// Indique si ce format inclut le flux audio
    pub has_audio: bool,
    /// URL directe de la ressource vidéo/stream si résolue
    pub direct_url: Option<String>,
}

/// Métadonnées d'une vidéo après analyse de son URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoMetadata {
    /// Identifiant de la vidéo sur la plateforme source
    pub id: String,
    /// Titre de la vidéo
    pub title: String,
    /// Description ou résumé éventuel
    pub description: Option<String>,
    /// Auteur ou nom de la chaîne/créateur
    pub author: Option<String>,
    /// URL de la vignette (thumbnail)
    pub thumbnail_url: Option<String>,
    /// Durée de la vidéo en secondes
    pub duration_seconds: Option<u64>,
    /// Nom de la plateforme d'origine (ex: "YouTube", "MoviBox", "DirectStream")
    pub source_platform: String,
    /// URL d'origine fournie par l'utilisateur
    pub source_url: String,
    /// Liste des formats et qualités disponibles au téléchargement
    pub available_formats: Vec<VideoFormat>,
}

/// Requête de lancement d'un téléchargement soumise depuis le frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadRequest {
    /// URL de la vidéo à télécharger
    pub url: String,
    /// Identifiant du format choisi (ou "best" par défaut)
    pub format_id: Option<String>,
    /// Répertoire de destination personnalisé (ou dossier Téléchargements par défaut)
    pub output_dir: Option<String>,
    /// Nom personnalisé pour le fichier final (optionnel)
    pub custom_filename: Option<String>,
}

/// Statut d'avancement d'un téléchargement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// Données émises en temps réel vers le frontend via l'événement `download-progress`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadProgressPayload {
    /// Identifiant unique du téléchargement
    pub download_id: String,
    /// État actuel du processus
    pub status: DownloadStatus,
    /// Pourcentage de progression (de 0.0 à 100.0)
    pub progress_percentage: f64,
    /// Quantité d'octets actuellement reçue
    pub downloaded_bytes: u64,
    /// Taille totale attendue en octets (si connue)
    pub total_bytes: Option<u64>,
    /// Vitesse de téléchargement instantanée en octets par seconde
    pub speed_bytes_per_sec: u64,
    /// Estimation du temps restant en secondes
    pub eta_seconds: Option<u64>,
}

/// Résultat retourné à l'issue d'un téléchargement réussi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadResult {
    /// Identifiant unique du téléchargement
    pub download_id: String,
    /// Chemin absolu du fichier enregistré sur le disque
    pub file_path: String,
    /// Taille totale du fichier téléchargé en octets
    pub file_size_bytes: u64,
    /// Titre de la vidéo enregistrée
    pub title: String,
}
