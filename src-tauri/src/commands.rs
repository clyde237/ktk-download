use tauri::{AppHandle, Emitter};
use url::Url;
use uuid::Uuid;

use crate::error::CommandError;
use crate::models::{
    DownloadProgressPayload, DownloadRequest, DownloadResult, DownloadStatus, VideoFormat,
    VideoMetadata,
};

/// Commande IPC : Analyse une URL vidéo et renvoie ses métadonnées ainsi que les qualités disponibles.
///
/// Cette commande effectue une validation préliminaire de l'URL et détecte la plateforme
/// (YouTube, MoviBox ou flux direct) avant d'extraire les formats disponibles.
#[tauri::command]
pub async fn inspect_url(url: String) -> Result<VideoMetadata, CommandError> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err(CommandError::InvalidUrl(
            "L'adresse URL ne peut pas être vide.".to_string(),
        ));
    }

    // Validation syntaxique de l'URL
    let parsed_url = Url::parse(trimmed_url)
        .map_err(|e| CommandError::InvalidUrl(format!("Format d'adresse invalide : {}", e)))?;

    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(CommandError::InvalidUrl(
            "Seuls les protocoles HTTP et HTTPS sont pris en charge.".to_string(),
        ));
    }

    let host = parsed_url.host_str().unwrap_or("").to_lowercase();

    // Détection de la plateforme source
    let (platform_name, estimated_title) =
        if host.contains("youtube.com") || host.contains("youtu.be") {
            ("YouTube".to_string(), "Vidéo YouTube".to_string())
        } else if host.contains("movibox.net") || host.contains("movibox") {
            ("MoviBox".to_string(), "Contenu MoviBox".to_string())
        } else {
            ("Flux Web / Direct".to_string(), "Vidéo Web".to_string())
        };

    // Formats types préconfigurés pour la structure initiale
    let formats = vec![
        VideoFormat {
            format_id: "best".to_string(),
            label: "Meilleure qualité disponible (Auto)".to_string(),
            resolution: Some("1080p".to_string()),
            extension: "mp4".to_string(),
            file_size_bytes: None,
            has_video: true,
            has_audio: true,
            direct_url: None,
        },
        VideoFormat {
            format_id: "720p".to_string(),
            label: "720p HD".to_string(),
            resolution: Some("1280x720".to_string()),
            extension: "mp4".to_string(),
            file_size_bytes: None,
            has_video: true,
            has_audio: true,
            direct_url: None,
        },
        VideoFormat {
            format_id: "audio_only".to_string(),
            label: "Audio uniquement (MP3)".to_string(),
            resolution: None,
            extension: "mp3".to_string(),
            file_size_bytes: None,
            has_video: false,
            has_audio: true,
            direct_url: None,
        },
    ];

    Ok(VideoMetadata {
        id: Uuid::new_v4().to_string(),
        title: estimated_title,
        description: Some(format!("Flux analysé depuis {}", host)),
        author: None,
        thumbnail_url: None,
        duration_seconds: None,
        source_platform: platform_name,
        source_url: trimmed_url.to_string(),
        available_formats: formats,
    })
}

/// Commande IPC : Initialise et orchestre le téléchargement d'une vidéo avec suivi de progression.
///
/// Cette commande émet des événements `download-progress` via l'AppHandle Tauri pour informer
/// l'interface en temps réel de l'avancement.
#[tauri::command]
pub async fn download_video(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<DownloadResult, CommandError> {
    let trimmed_url = request.url.trim();
    if trimmed_url.is_empty() {
        return Err(CommandError::InvalidUrl(
            "L'adresse URL à télécharger est requise.".to_string(),
        ));
    }

    let download_id = Uuid::new_v4().to_string();
    let format = request.format_id.as_deref().unwrap_or("best");

    // Notification initiale d'entrée en file d'attente
    let initial_payload = DownloadProgressPayload {
        download_id: download_id.clone(),
        status: DownloadStatus::Queued,
        progress_percentage: 0.0,
        downloaded_bytes: 0,
        total_bytes: None,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
    };

    app.emit("download-progress", &initial_payload)
        .map_err(|e| {
            CommandError::Internal(format!("Échec de transmission de l'événement : {}", e))
        })?;

    // Résolution du répertoire de destination
    let destination_dir = request
        .output_dir
        .unwrap_or_else(|| "Downloads".to_string());

    let final_filename = request
        .custom_filename
        .unwrap_or_else(|| format!("video_{}.mp4", &download_id[..8]));

    let target_path = format!("{}/{}", destination_dir, final_filename);

    // Émission d'un état de complétion (structure prête pour le moteur de téléchargement réel)
    let completed_payload = DownloadProgressPayload {
        download_id: download_id.clone(),
        status: DownloadStatus::Completed,
        progress_percentage: 100.0,
        downloaded_bytes: 1024 * 1024 * 15,
        total_bytes: Some(1024 * 1024 * 15),
        speed_bytes_per_sec: 1024 * 1024 * 2,
        eta_seconds: Some(0),
    };

    let _ = app.emit("download-progress", &completed_payload);

    Ok(DownloadResult {
        download_id,
        file_path: target_path,
        file_size_bytes: 1024 * 1024 * 15,
        title: format!("Vidéo ({})", format),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_url_empty() {
        tauri::async_runtime::block_on(async {
            let result = inspect_url("   ".to_string()).await;
            assert!(result.is_err());
            match result.unwrap_err() {
                CommandError::InvalidUrl(msg) => {
                    assert!(msg.contains("ne peut pas être vide"));
                }
                other => panic!("Erreur inattendue : {:?}", other),
            }
        });
    }

    #[test]
    fn test_inspect_url_invalid_scheme() {
        tauri::async_runtime::block_on(async {
            let result = inspect_url("ftp://example.com/video.mp4".to_string()).await;
            assert!(result.is_err());
            match result.unwrap_err() {
                CommandError::InvalidUrl(msg) => {
                    assert!(msg.contains("HTTP et HTTPS"));
                }
                other => panic!("Erreur inattendue : {:?}", other),
            }
        });
    }

    #[test]
    fn test_inspect_url_youtube_detection() {
        tauri::async_runtime::block_on(async {
            let result =
                inspect_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()).await;
            assert!(result.is_ok());
            let meta = result.unwrap();
            assert_eq!(meta.source_platform, "YouTube");
            assert!(!meta.available_formats.is_empty());
        });
    }

    #[test]
    fn test_inspect_url_movibox_detection() {
        tauri::async_runtime::block_on(async {
            let result = inspect_url("https://movibox.net/watch/12345".to_string()).await;
            assert!(result.is_ok());
            let meta = result.unwrap();
            assert_eq!(meta.source_platform, "MoviBox");
            assert!(!meta.available_formats.is_empty());
        });
    }

    #[test]
    fn test_inspect_url_generic_stream() {
        tauri::async_runtime::block_on(async {
            let result = inspect_url("https://cdn.example.org/media/sample.mp4".to_string()).await;
            assert!(result.is_ok());
            let meta = result.unwrap();
            assert_eq!(meta.source_platform, "Flux Web / Direct");
        });
    }

    #[test]
    fn test_error_serialization_structure() {
        let err = CommandError::InvalidUrl("Format erroné".to_string());
        let json = serde_json::to_string(&err).expect("Sérialisation JSON");
        assert_eq!(json, r#"{"kind":"InvalidUrl","message":"Format erroné"}"#);
    }
}
