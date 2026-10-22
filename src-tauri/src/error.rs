use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Types d'erreurs dédiés aux commandes IPC du backend Tauri.
///
/// Ces erreurs sont sérialisées avec une structure discriminée `{"kind": "...", "message": "..."}`
/// afin d'offrir au frontend TypeScript un typage strict et une gestion précise des retours d'erreurs.
#[derive(Debug, Error, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "message")]
pub enum CommandError {
    #[error("L'URL fournie est invalide : {0}")]
    InvalidUrl(String),

    #[error("Plateforme vidéo non prise en charge : {0}")]
    UnsupportedPlatform(String),

    #[error("Erreur de communication réseau : {0}")]
    NetworkError(String),

    #[error("Échec lors de l'extraction des informations vidéo : {0}")]
    ExtractionFailed(String),

    #[error("Erreur d'accès au système de fichiers : {0}")]
    FileSystemError(String),

    #[error("Échec lors du téléchargement de la vidéo : {0}")]
    DownloadFailed(String),

    #[error("Le téléchargement a été annulé par l'utilisateur.")]
    DownloadCancelled,

    #[error("Une erreur interne inattendue est survenue : {0}")]
    Internal(String),
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
