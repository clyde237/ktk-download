use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Types d'erreurs dédiés aux commandes IPC du backend Tauri.
///
/// Ces erreurs sont sérialisées avec une structure discriminée `{"kind": "...", "message": "..."}`
/// afin d'offrir au frontend TypeScript un typage strict et une gestion précise des retours d'erreurs.
#[derive(Debug, Error, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "message")]
pub enum CommandError {
    #[error("Adresse URL invalide : {0}")]
    InvalidUrl(String),

    #[error("Plateforme vidéo non prise en charge : {0}")]
    UnsupportedPlatform(String),

    #[error("Délai d'attente réseau dépassé : {0}")]
    NetworkTimeout(String),

    #[error("Erreur de connexion réseau : {0}")]
    NetworkError(String),

    #[error("Vidéo introuvable ou inaccessible : {0}")]
    VideoNotFound(String),

    #[error("Échec lors de l'extraction des informations : {0}")]
    ExtractionFailed(String),

    #[error("Erreur du système de fichiers : {0}")]
    FileSystemError(String),

    #[error("Échec lors du téléchargement : {0}")]
    DownloadFailed(String),

    #[error("Le téléchargement a été annulé par l'utilisateur.")]
    DownloadCancelled,

    #[error("Le téléchargement a été mis en pause.")]
    DownloadPaused,

    #[error("Une erreur inattendue est survenue : {0}")]
    Internal(String),
}

impl CommandError {
    /// Convertit un code de statut HTTP en une erreur explicite pour l'utilisateur.
    pub fn from_http_status(status: reqwest::StatusCode) -> Self {
        let code = status.as_u16();
        match code {
            403 => Self::DownloadFailed(
                "Accès refusé par le serveur hébergeur (erreur 403). La vidéo est peut-être privée ou restreinte.".to_string(),
            ),
            404 => Self::VideoNotFound(
                "La vidéo demandée est introuvable ou a été supprimée (erreur 404).".to_string(),
            ),
            429 => Self::NetworkError(
                "Trop de requêtes envoyées à la plateforme (erreur 429). Veuillez patienter quelques instants avant de réessayer.".to_string(),
            ),
            500..=599 => Self::NetworkError(format!(
                "Le serveur hébergeant la vidéo rencontre un dysfonctionnement temporaire (erreur {}).",
                code
            )),
            _ => Self::DownloadFailed(format!(
                "Le serveur distant a renvoyé un statut HTTP inattendu (code {}).",
                code
            )),
        }
    }

    /// Analyse les messages d'erreur retournés par yt-dlp pour fournir une explication claire.
    pub fn from_ytdlp_stderr(stderr: &str) -> Self {
        let lower = stderr.to_lowercase();
        // Avant « not found » : « ffmpeg not found » ne signifie pas que la vidéo est introuvable.
        if lower.contains("ffmpeg")
            && (lower.contains("not found") || lower.contains("not installed"))
        {
            Self::DownloadFailed(
                "ffmpeg est requis pour ce format (fusion audio/vidéo ou conversion MP3). Installez ffmpeg puis réessayez.".to_string(),
            )
        } else if lower.contains("video unavailable")
            || lower.contains("does not exist")
            || lower.contains("not found")
        {
            Self::VideoNotFound(
                "Cette vidéo n'est plus disponible ou a été supprimée du site.".to_string(),
            )
        } else if lower.contains("not a bot") {
            // « Sign in to confirm you're not a bot » : blocage temporaire, pas une vidéo privée.
            Self::DownloadFailed(
                "YouTube a temporairement bloqué la requête (vérification anti-robot). Réessayez dans quelques minutes, en espaçant les téléchargements.".to_string(),
            )
        } else if lower.contains("confirm your age") || lower.contains("age-restricted") {
            Self::DownloadFailed(
                "Vidéo soumise à une limite d'âge : YouTube exige une connexion pour la lire."
                    .to_string(),
            )
        } else if lower.contains("private video")
            || lower.contains("members-only")
            || lower.contains("sign in")
        {
            Self::DownloadFailed(
                "Cette vidéo est privée, réservée aux membres ou requiert une connexion."
                    .to_string(),
            )
        } else if lower.contains("unsupported url") || lower.contains("no video formats") {
            Self::UnsupportedPlatform("L'adresse fournie ne correspond à aucune source vidéo accessible sur cette plateforme.".to_string())
        } else if lower.contains("timed out") || lower.contains("timeout") {
            Self::NetworkTimeout("Le délai de communication avec la plateforme vidéo a expiré. Vérifiez votre connexion.".to_string())
        } else if lower.contains("network is unreachable")
            || lower.contains("connection refused")
            || lower.contains("name resolution")
        {
            Self::NetworkError(
                "Impossible de joindre la plateforme vidéo. Vérifiez votre accès à Internet."
                    .to_string(),
            )
        } else {
            let clean_msg = stderr
                .lines()
                .find(|l| l.contains("ERROR:"))
                .and_then(|l| l.strip_prefix("ERROR:"))
                .map(|s| s.trim())
                .unwrap_or_else(|| stderr.trim());

            Self::ExtractionFailed(if clean_msg.is_empty() {
                "Impossible d'analyser le contenu vidéo de cette page.".to_string()
            } else {
                clean_msg.to_string()
            })
        }
    }
}

impl From<reqwest::Error> for CommandError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::NetworkTimeout(
                "Le délai de connexion au serveur distant a expiré. Vérifiez votre réseau."
                    .to_string(),
            )
        } else if err.is_connect() {
            Self::NetworkError("Connexion impossible au serveur distant. Vérifiez votre accès Internet ou la validité du lien.".to_string())
        } else if let Some(status) = err.status() {
            Self::from_http_status(status)
        } else {
            Self::NetworkError(format!("Erreur de communication réseau : {}", err))
        }
    }
}

impl From<url::ParseError> for CommandError {
    fn from(err: url::ParseError) -> Self {
        Self::InvalidUrl(err.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        Self::FileSystemError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_http_status_codes() {
        assert!(matches!(
            CommandError::from_http_status(reqwest::StatusCode::NOT_FOUND),
            CommandError::VideoNotFound(_)
        ));

        assert!(matches!(
            CommandError::from_http_status(reqwest::StatusCode::FORBIDDEN),
            CommandError::DownloadFailed(_)
        ));

        assert!(matches!(
            CommandError::from_http_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            CommandError::NetworkError(_)
        ));

        assert!(matches!(
            CommandError::from_http_status(reqwest::StatusCode::BAD_GATEWAY),
            CommandError::NetworkError(_)
        ));
    }

    #[test]
    fn test_from_ytdlp_stderr_parsing() {
        let unavailable = "ERROR: [youtube] dQw4w9WgXcQ: Video unavailable";
        assert!(matches!(
            CommandError::from_ytdlp_stderr(unavailable),
            CommandError::VideoNotFound(_)
        ));

        let private_video = "ERROR: [youtube] 1234: Private video. Sign in to verify.";
        assert!(matches!(
            CommandError::from_ytdlp_stderr(private_video),
            CommandError::DownloadFailed(_)
        ));

        let unsupported = "ERROR: Unsupported URL: https://example.com/not-video";
        assert!(matches!(
            CommandError::from_ytdlp_stderr(unsupported),
            CommandError::UnsupportedPlatform(_)
        ));

        let timeout = "ERROR: Connection timed out after 30 seconds";
        assert!(matches!(
            CommandError::from_ytdlp_stderr(timeout),
            CommandError::NetworkTimeout(_)
        ));

        let bot_check = "ERROR: [youtube] B5QJZFYQ2Qs: Sign in to confirm you’re not a bot. Use --cookies-from-browser or --cookies for the authentication.";
        match CommandError::from_ytdlp_stderr(bot_check) {
            CommandError::DownloadFailed(msg) => assert!(msg.contains("anti-robot")),
            other => panic!("Erreur inattendue : {:?}", other),
        }

        let age_gate = "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate for some users.";
        match CommandError::from_ytdlp_stderr(age_gate) {
            CommandError::DownloadFailed(msg) => assert!(msg.contains("limite d'âge")),
            other => panic!("Erreur inattendue : {:?}", other),
        }

        let missing_ffmpeg = "ERROR: Postprocessing: ffprobe and ffmpeg not found. Please install or provide the path using --ffmpeg-location";
        match CommandError::from_ytdlp_stderr(missing_ffmpeg) {
            CommandError::DownloadFailed(msg) => assert!(msg.contains("ffmpeg")),
            other => panic!("Erreur inattendue : {:?}", other),
        }
    }

    #[test]
    fn test_error_json_structure() {
        let err = CommandError::NetworkTimeout("Connexion perdue".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"NetworkTimeout","message":"Connexion perdue"}"#
        );
    }
}
