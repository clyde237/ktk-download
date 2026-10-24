use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::downloader::{self, DownloadRegistry};
use crate::error::CommandError;
use crate::extractor;
use crate::models::{
    DownloadProgressPayload, DownloadRequest, DownloadResult, DownloadStatus, ValidatedUrlPayload,
    VideoMetadata,
};
use crate::quality::QualityPreset;
use crate::validator::{self, VideoPlatform};

/// Commande IPC : Valide et assainit instantanément une URL sans bloquer l'interface.
///
/// Permet au frontend de vérifier en temps réel la validité du format et d'identifier la plateforme
/// (YouTube, Vimeo, flux direct, etc.) dès la frappe ou le collage.
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

/// Commande IPC : Annule un téléchargement en cours ou en pause via son identifiant.
///
/// Renvoie `false` si le téléchargement n'est ni actif ni en pause (déjà terminé ou inconnu).
#[tauri::command]
pub async fn cancel_download(
    registry: State<'_, DownloadRegistry>,
    download_id: String,
) -> Result<bool, CommandError> {
    if registry.cancel(&download_id) {
        return Ok(true);
    }
    // En pause : plus de processus à arrêter, seulement ses fichiers partiels à supprimer.
    match registry.take_paused(&download_id) {
        Some(target_path) => {
            extractor::discard_partial_download(&target_path).await;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Commande IPC : Met en pause un téléchargement en cours en conservant ses fichiers partiels.
///
/// La reprise se fait en relançant `download_video` avec le même identifiant. Renvoie `false`
/// si le téléchargement n'est pas actif.
#[tauri::command]
pub fn pause_download(
    registry: State<'_, DownloadRegistry>,
    download_id: String,
) -> Result<bool, CommandError> {
    Ok(registry.pause(&download_id))
}

/// Commande IPC : Analyse une URL vidéo et renvoie ses métadonnées ainsi que les qualités disponibles.
///
/// Cette commande valide et assainit d'abord l'URL, puis utilise le moteur d'extraction `yt-dlp`
/// (avec analyse HTML/HLS de secours pour les fichiers et lecteurs web génériques) pour résoudre
/// dynamiquement les informations vidéo. La règle est la même qu'au téléchargement : une page de
/// plateforme n'est jamais analysée ni téléchargée en direct.
#[tauri::command]
pub async fn inspect_url(url: String) -> Result<VideoMetadata, CommandError> {
    let validated = validator::validate_and_sanitize_url(&url)?;
    extractor::extract_metadata(
        &validated.canonical_url,
        allows_direct_stream(&validated.platform),
    )
    .await
}

/// Commande IPC : Initialise et orchestre le téléchargement d'une vidéo avec suivi de progression.
///
/// Valide rigoureusement l'URL avant de lancer le téléchargement avec `yt-dlp` ou le moteur
/// de streaming asynchrone natif.
#[tauri::command]
pub async fn download_video<R: tauri::Runtime>(
    app: AppHandle<R>,
    registry: State<'_, DownloadRegistry>,
    request: DownloadRequest,
) -> Result<DownloadResult, CommandError> {
    let validated = validator::validate_and_sanitize_url(&request.url)?;
    let output_dir = validate_output_dir(request.output_dir.as_deref())?;
    let quality = QualityPreset::parse(request.format_id.as_deref().unwrap_or_default())?;

    let download_id = resolve_download_id(request.download_id.as_deref())?;
    if registry.is_active(&download_id) {
        return Err(CommandError::Internal(
            "Un téléchargement portant cet identifiant est déjà en cours.".to_string(),
        ));
    }
    let control = registry.register(&download_id);
    // Reprise après une pause : mêmes fichiers partiels, donc même chemin cible.
    let resumed_target = registry.take_paused(&download_id);

    let extension = if quality.is_audio_only() {
        "mp3"
    } else {
        "mp4"
    };

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
    let target_path = resumed_target.unwrap_or_else(|| {
        downloader::resolve_output_path(
            output_dir.as_deref(),
            request.custom_filename.as_deref(),
            &download_id[..8],
            extension,
        )
    });

    // Le streaming direct enregistre le fichier tel quel : il ne sait ni plafonner la qualité
    // ni extraire l'audio, il n'est donc possible qu'avec la qualité par défaut.
    let direct_stream_allowed =
        allows_direct_stream(&validated.platform) && quality == QualityPreset::Best;
    let stream_directly = || {
        downloader::download_stream(
            &app,
            &download_id,
            &validated.canonical_url,
            &target_path,
            Some(Arc::clone(&control)),
        )
    };

    // yt-dlp en priorité ; le streaming direct seulement pour un fichier média servi tel quel
    let download_result = if extractor::find_ytdlp_binary().is_some() {
        match extractor::download_with_ytdlp(
            &app,
            &download_id,
            &validated.canonical_url,
            quality,
            &target_path,
            Some(Arc::clone(&control)),
        )
        .await
        {
            Ok(file) => Ok(file),
            // Une annulation ou une pause volontaire ne doit pas relancer un autre moteur.
            Err(CommandError::DownloadCancelled) => Err(CommandError::DownloadCancelled),
            Err(CommandError::DownloadPaused) => Err(CommandError::DownloadPaused),
            Err(e) if direct_stream_allowed => {
                log::warn!(
                    "Échec yt-dlp ({:?}), tentative avec le moteur de streaming direct...",
                    e
                );
                stream_directly()
                    .await
                    .map(|bytes| (target_path.clone(), bytes))
            }
            Err(e) => Err(e),
        }
    } else if direct_stream_allowed {
        stream_directly()
            .await
            .map(|bytes| (target_path.clone(), bytes))
    } else if allows_direct_stream(&validated.platform) {
        Err(CommandError::DownloadFailed(
            "yt-dlp est requis pour choisir une qualité ou extraire l'audio, mais il est introuvable sur le système.".to_string(),
        ))
    } else {
        Err(CommandError::DownloadFailed(format!(
            "yt-dlp est requis pour télécharger depuis {}, mais il est introuvable sur le système.",
            validated.platform.as_str()
        )))
    };

    registry.unregister(&download_id);
    if matches!(download_result, Err(CommandError::DownloadPaused)) {
        registry.remember_paused(&download_id, target_path.clone());
    }

    let (file_path, total_bytes) = download_result?;
    let file_path_str = file_path.to_string_lossy().to_string();
    let title = file_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("Vidéo ({})", quality.id()));

    Ok(DownloadResult {
        download_id,
        file_path: file_path_str,
        file_size_bytes: total_bytes,
        title,
    })
}

/// Le streaming direct enregistre la réponse HTTP telle quelle : il ne convient qu'à un fichier
/// média servi directement, jamais à la page d'une plateforme vidéo (YouTube, Vimeo…), dont
/// seul yt-dlp sait extraire le flux.
fn allows_direct_stream(platform: &VideoPlatform) -> bool {
    matches!(platform, VideoPlatform::GenericWeb(_))
}

/// Vérifie le dossier de destination choisi dans l'interface avant toute écriture.
///
/// Le chemin vient du frontend : il doit être absolu et désigner un dossier existant. Aucun dossier
/// n'est créé à partir de cette valeur ; `None` (ou une chaîne vide) garde le dossier par défaut.
fn validate_output_dir(requested: Option<&str>) -> Result<Option<String>, CommandError> {
    let Some(raw) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    let path = std::path::Path::new(raw);
    if !path.is_absolute() {
        return Err(CommandError::FileSystemError(
            "Le dossier de destination doit être un chemin absolu.".to_string(),
        ));
    }
    if !path.is_dir() {
        return Err(CommandError::FileSystemError(format!(
            "Le dossier de destination est introuvable : {}",
            raw
        )));
    }

    Ok(Some(raw.to_string()))
}

/// Valide l'identifiant fourni par le frontend (UUID) ou en génère un nouveau.
///
/// L'identifiant est renvoyé sous forme canonique (minuscules avec tirets), identique à celle
/// produite par `crypto.randomUUID()`, pour que le frontend reconnaisse ses événements.
fn resolve_download_id(requested: Option<&str>) -> Result<String, CommandError> {
    match requested {
        Some(raw) => Uuid::parse_str(raw.trim())
            .map(|id| id.to_string())
            .map_err(|_| {
                CommandError::Internal("Identifiant de téléchargement invalide.".to_string())
            }),
        None => Ok(Uuid::new_v4().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_stream_only_for_generic_web() {
        assert!(allows_direct_stream(&VideoPlatform::GenericWeb(
            "cdn.example.org".to_string()
        )));
        assert!(!allows_direct_stream(&VideoPlatform::YouTube));
        assert!(!allows_direct_stream(&VideoPlatform::Vimeo));
    }

    #[test]
    fn test_validate_output_dir_defaults_when_empty() {
        assert_eq!(validate_output_dir(None).unwrap(), None);
        assert_eq!(validate_output_dir(Some("   ")).unwrap(), None);
    }

    #[test]
    fn test_validate_output_dir_accepts_existing_absolute_dir() {
        let dir = std::env::temp_dir();
        let dir_str = dir.to_str().unwrap();
        assert_eq!(
            validate_output_dir(Some(dir_str)).unwrap(),
            Some(dir_str.to_string())
        );
    }

    #[test]
    fn test_validate_output_dir_rejects_relative_and_missing() {
        assert!(matches!(
            validate_output_dir(Some("../Videos")),
            Err(CommandError::FileSystemError(_))
        ));
        let missing = std::env::temp_dir().join("ktk-download-dossier-inexistant-7f3a");
        assert!(matches!(
            validate_output_dir(missing.to_str()),
            Err(CommandError::FileSystemError(_))
        ));
    }

    #[test]
    fn test_resolve_download_id_keeps_client_uuid() {
        let id = "3f2b8c1e-9d4a-4e6b-8f1a-2c3d4e5f6a7b";
        assert_eq!(resolve_download_id(Some(id)).unwrap(), id);
    }

    #[test]
    fn test_resolve_download_id_rejects_invalid_value() {
        assert!(matches!(
            resolve_download_id(Some("../../etc/passwd")),
            Err(CommandError::Internal(_))
        ));
    }

    #[test]
    fn test_resolve_download_id_generates_when_missing() {
        let id = resolve_download_id(None).unwrap();
        assert!(Uuid::parse_str(&id).is_ok());
    }

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
    fn test_inspect_url_without_video_is_error() {
        // `.invalid` ne se résout jamais (RFC 2606) : aucune métadonnée ne doit être inventée.
        tauri::async_runtime::block_on(async {
            let result =
                inspect_url("https://ktk-download.invalid/media/sample.mp4".to_string()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_error_serialization_structure() {
        let err = CommandError::InvalidUrl("Format erroné".to_string());
        let json = serde_json::to_string(&err).expect("Sérialisation JSON");
        assert_eq!(json, r#"{"kind":"InvalidUrl","message":"Format erroné"}"#);
    }
}
