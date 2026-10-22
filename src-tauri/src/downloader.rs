use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::AsyncWriteExt;

use crate::error::CommandError;
use crate::models::{DownloadProgressPayload, DownloadStatus};

/// Résout et sécurise le chemin de destination pour l'enregistrement d'un fichier téléchargé.
///
/// Assainit le nom du fichier pour prévenir toute attaque par traversée de répertoire (path traversal)
/// et utilise le dossier standard `Downloads` du système en l'absence de répertoire spécifié.
pub fn resolve_output_path(
    output_dir: Option<&str>,
    custom_filename: Option<&str>,
    fallback_id: &str,
    default_extension: &str,
) -> PathBuf {
    let base_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        dirs::download_dir().unwrap_or_else(|| PathBuf::from("Downloads"))
    };

    let raw_name = if let Some(name) = custom_filename {
        name.trim()
    } else {
        ""
    };

    let filename = if raw_name.is_empty() {
        format!("video_{}.{}", fallback_id, default_extension)
    } else {
        // Assainissement des caractères dangereux pour le système de fichiers
        let sanitized: String = raw_name
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                other => other,
            })
            .collect();

        // Empêcher les tentatives de traversée de chemin
        let safe_name = sanitized.trim_matches('.').trim();
        if safe_name.is_empty() {
            format!("video_{}.{}", fallback_id, default_extension)
        } else if !safe_name.contains('.') {
            format!("{}.{}", safe_name, default_extension)
        } else {
            safe_name.to_string()
        }
    };

    base_dir.join(filename)
}

/// Télécharge un flux vidéo ou direct via HTTP en mode streaming asynchrone.
///
/// Émet des événements `download-progress` régulés (throttled) vers le frontend via Tauri IPC
/// avec mise à jour du pourcentage, vitesse de téléchargement et estimation du temps restant.
pub async fn download_stream<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    source_url: &str,
    target_file_path: &Path,
) -> Result<u64, CommandError> {
    // Création des répertoires parents si nécessaires
    if let Some(parent) = target_file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CommandError::FileSystemError(e.to_string()))?;
    }

    // Configuration du client HTTP avec en-têtes standard
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(3600)) // 1 heure max pour les flux longs
        .build()
        .map_err(|e| CommandError::NetworkError(format!("Échec de configuration HTTP : {}", e)))?;

    // Émission du statut de démarrage
    let start_payload = DownloadProgressPayload {
        download_id: download_id.to_string(),
        status: DownloadStatus::Downloading,
        progress_percentage: 0.0,
        downloaded_bytes: 0,
        total_bytes: None,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
    };
    let _ = app.emit("download-progress", &start_payload);

    // Requête HTTP GET avec corps en streaming
    let response = client.get(source_url).send().await.map_err(|e| {
        let _ = app.emit(
            "download-progress",
            &DownloadProgressPayload {
                download_id: download_id.to_string(),
                status: DownloadStatus::Failed,
                progress_percentage: 0.0,
                downloaded_bytes: 0,
                total_bytes: None,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
            },
        );
        CommandError::NetworkError(format!("Erreur lors de la requête HTTP : {}", e))
    })?;

    let status = response.status();
    if !status.is_success() {
        let err_msg = format!("Le serveur a renvoyé un statut HTTP {}", status);
        let _ = app.emit(
            "download-progress",
            &DownloadProgressPayload {
                download_id: download_id.to_string(),
                status: DownloadStatus::Failed,
                progress_percentage: 0.0,
                downloaded_bytes: 0,
                total_bytes: None,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
            },
        );
        return Err(CommandError::DownloadFailed(err_msg));
    }

    let total_bytes = response.content_length();

    // Création du fichier cible sur disque
    let mut file = tokio::fs::File::create(target_file_path)
        .await
        .map_err(|e| {
            let _ = app.emit(
                "download-progress",
                &DownloadProgressPayload {
                    download_id: download_id.to_string(),
                    status: DownloadStatus::Failed,
                    progress_percentage: 0.0,
                    downloaded_bytes: 0,
                    total_bytes,
                    speed_bytes_per_sec: 0,
                    eta_seconds: None,
                },
            );
            CommandError::FileSystemError(format!("Impossible de créer le fichier cible : {}", e))
        })?;

    let mut stream = response.bytes_stream();
    let mut downloaded_bytes: u64 = 0;

    let mut last_emit_instant = Instant::now();
    let mut last_emit_bytes: u64 = 0;

    // Lecture asynchrone des blocs de données
    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = app.emit(
                    "download-progress",
                    &DownloadProgressPayload {
                        download_id: download_id.to_string(),
                        status: DownloadStatus::Failed,
                        progress_percentage: 0.0,
                        downloaded_bytes,
                        total_bytes,
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                    },
                );
                return Err(CommandError::NetworkError(format!(
                    "Interruption de flux : {}",
                    e
                )));
            }
        };

        if let Err(e) = file.write_all(&chunk).await {
            let _ = app.emit(
                "download-progress",
                &DownloadProgressPayload {
                    download_id: download_id.to_string(),
                    status: DownloadStatus::Failed,
                    progress_percentage: 0.0,
                    downloaded_bytes,
                    total_bytes,
                    speed_bytes_per_sec: 0,
                    eta_seconds: None,
                },
            );
            return Err(CommandError::FileSystemError(format!(
                "Erreur d'écriture sur le disque : {}",
                e
            )));
        }

        downloaded_bytes += chunk.len() as u64;

        let now = Instant::now();
        let elapsed_duration = now.duration_since(last_emit_instant);

        // Régulation d'émission : au maximum toutes les 100ms
        if elapsed_duration >= Duration::from_millis(100) {
            let delta_bytes = downloaded_bytes.saturating_sub(last_emit_bytes);
            let speed = if elapsed_duration.as_secs_f64() > 0.0 {
                (delta_bytes as f64 / elapsed_duration.as_secs_f64()) as u64
            } else {
                0
            };

            let (pct, eta) = if let Some(total) = total_bytes {
                let p = if total > 0 {
                    ((downloaded_bytes as f64 / total as f64) * 100.0).min(99.9)
                } else {
                    0.0
                };
                let remaining = total.saturating_sub(downloaded_bytes);
                let est_time = remaining.checked_div(speed);
                (p, est_time)
            } else {
                (0.0, None)
            };

            let payload = DownloadProgressPayload {
                download_id: download_id.to_string(),
                status: DownloadStatus::Downloading,
                progress_percentage: (pct * 10.0).round() / 10.0,
                downloaded_bytes,
                total_bytes,
                speed_bytes_per_sec: speed,
                eta_seconds: eta,
            };

            let _ = app.emit("download-progress", &payload);

            last_emit_instant = now;
            last_emit_bytes = downloaded_bytes;
        }
    }

    // Forçage de l'écriture sur le disque
    file.flush().await.map_err(|e| {
        CommandError::FileSystemError(format!("Échec de synchronisation disque : {}", e))
    })?;

    // Émission du statut de fin de téléchargement (100%)
    let completed_payload = DownloadProgressPayload {
        download_id: download_id.to_string(),
        status: DownloadStatus::Completed,
        progress_percentage: 100.0,
        downloaded_bytes,
        total_bytes: Some(downloaded_bytes),
        speed_bytes_per_sec: 0,
        eta_seconds: Some(0),
    };

    let _ = app.emit("download-progress", &completed_payload);

    Ok(downloaded_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_output_path_sanitization() {
        let path = resolve_output_path(
            Some("/tmp/downloads"),
            Some("video:test/demo*?.mp4"),
            "12345678",
            "mp4",
        );
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "video_test_demo__.mp4");
        assert_eq!(path.parent().unwrap(), Path::new("/tmp/downloads"));
    }

    #[test]
    fn test_resolve_output_path_fallback() {
        let path = resolve_output_path(Some("/tmp"), None, "abcdef12", "mp4");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "video_abcdef12.mp4");
    }

    #[test]
    fn test_resolve_output_path_extension_addition() {
        let path = resolve_output_path(Some("/tmp"), Some("my_vacation"), "abcdef12", "mp4");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "my_vacation.mp4");
    }
}
