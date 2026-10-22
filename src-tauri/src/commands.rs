use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::downloader;
use crate::error::CommandError;
use crate::extractor;
use crate::models::{
    DownloadProgressPayload, DownloadRequest, DownloadResult, DownloadStatus, ValidatedUrlPayload,
    VideoMetadata,
};
use crate::validator;

/// Commande IPC : Valide et assainit instantanément une URL sans bloquer l'interface.
///
/// Permet au frontend de vérifier en temps réel la validité du format et d'identifier la plateforme
/// (YouTube, MoviBox, etc.) dès la frappe ou le collage.
#[tauri::command]
pub fn validate_url(url: String) -> Result<ValidatedUrlPayload, CommandError> {
    let validated = validator::validate_and_sanitize_url(&url)?;
    Ok(ValidatedUrlPayload {
        is_valid: true,
        canonical_url: validated.canonical_url,
        domain: validated.domain,
        platform: validated.platform.as_str().to_string(),
    })
}

/// Commande IPC : Analyse une URL vidéo et renvoie ses métadonnées ainsi que les qualités disponibles.
///
/// Cette commande valide et assainit d'abord l'URL, puis utilise le moteur d'extraction `yt-dlp`
/// (avec fallback HTML/HLS pour MoviBox et flux directs) pour résoudre dynamiquement les informations vidéo.
#[tauri::command]
pub async fn inspect_url(url: String) -> Result<VideoMetadata, CommandError> {
    let validated = validator::validate_and_sanitize_url(&url)?;
    extractor::extract_metadata(&validated.canonical_url).await
}

/// Commande IPC : Initialise et orchestre le téléchargement d'une vidéo avec suivi de progression.
///
/// Valide rigoureusement l'URL avant de lancer le téléchargement avec `yt-dlp` ou le moteur
/// de streaming asynchrone natif.
#[tauri::command]
pub async fn download_video(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<DownloadResult, CommandError> {
    let validated = validator::validate_and_sanitize_url(&request.url)?;

    let download_id = Uuid::new_v4().to_string();
    let format = request.format_id.as_deref().unwrap_or("best");
    let extension = if format == "audio_only" { "mp3" } else { "mp4" };

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

    let _ = app.emit("download-progress", &initial_payload);

    // Résolution du chemin de sortie sécurisé
    let target_path = downloader::resolve_output_path(
        request.output_dir.as_deref(),
        request.custom_filename.as_deref(),
        &download_id[..8],
        extension,
    );

    // Tentative de téléchargement avec yt-dlp si disponible
    let download_result = if extractor::find_ytdlp_binary().is_some() {
        match extractor::download_with_ytdlp(
            &app,
            &download_id,
            &validated.canonical_url,
            Some(format),
            &target_path,
        )
        .await
        {
            Ok(bytes) => Ok(bytes),
            Err(e) => {
                log::warn!(
                    "Échec yt-dlp ({:?}), tentative avec le moteur de streaming direct...",
                    e
                );
                downloader::download_stream(
                    &app,
                    &download_id,
                    &validated.canonical_url,
                    &target_path,
                )
                .await
            }
        }
    } else {
        downloader::download_stream(&app, &download_id, &validated.canonical_url, &target_path)
            .await
    };

    let total_bytes = download_result?;
    let file_path_str = target_path.to_string_lossy().to_string();
    let title = target_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("Vidéo ({})", format));

    Ok(DownloadResult {
        download_id,
        file_path: file_path_str,
        file_size_bytes: total_bytes,
        title,
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
