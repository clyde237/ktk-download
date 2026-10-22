use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::error::CommandError;
use crate::models::{DownloadProgressPayload, DownloadStatus};

/// Registre gérant l'état des téléchargements actifs et leurs jetons d'annulation.
#[derive(Debug, Default, Clone)]
pub struct DownloadRegistry {
    active_tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl DownloadRegistry {
    /// Enregistre un nouveau téléchargement actif et retourne son jeton d'annulation.
    pub fn register(&self, download_id: &str) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        let mut map = self.active_tokens.lock().unwrap();
        map.insert(download_id.to_string(), Arc::clone(&token));
        token
    }

    /// Signale l'annulation d'un téléchargement en cours par son identifiant.
    pub fn cancel(&self, download_id: &str) -> bool {
        let map = self.active_tokens.lock().unwrap();
        if let Some(token) = map.get(download_id) {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Retire un téléchargement du registre à sa conclusion (succès, échec ou annulation).
    pub fn unregister(&self, download_id: &str) {
        let mut map = self.active_tokens.lock().unwrap();
        map.remove(download_id);
    }

    /// Vérifie si un téléchargement est actuellement actif.
    pub fn is_active(&self, download_id: &str) -> bool {
        let map = self.active_tokens.lock().unwrap();
        map.contains_key(download_id)
    }
}

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

/// Pipeline de streaming asynchrone haute performance avec écriture disque non-bloquante et émission IPC.
///
/// Caractéristiques techniques du pipeline :
/// - Utilisation d'un tampon d'écriture asynchrone `BufWriter` (128 Ko) pour minimiser les appels système disque.
/// - Écriture temporaire dans un fichier `.part` garantissant l'atomicité du fichier final via renommage.
/// - Calcul lissé de la vitesse de téléchargement par moyenne mobile exponentielle (EMA).
/// - Estimation précise du temps restant (ETA).
/// - Régulation des émissions IPC (Throttling ~100 ms) pour une fluidité maximale de l'UI.
/// - Prise en charge native de l'interruption / annulation en temps réel via `cancel_token`.
pub async fn download_stream<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    source_url: &str,
    target_file_path: &Path,
    cancel_token: Option<Arc<AtomicBool>>,
) -> Result<u64, CommandError> {
    // 1. Création des répertoires parents si nécessaires
    if let Some(parent) = target_file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CommandError::FileSystemError(e.to_string()))?;
    }

    // 2. Fichier temporaire `.part` pour garantir l'intégrité
    let file_name = target_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let part_file_path = target_file_path.with_file_name(format!("{}.part", file_name));

    // 3. Configuration du client HTTP
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|e| CommandError::NetworkError(format!("Échec de configuration HTTP : {}", e)))?;

    // 4. Notification d'initialisation
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

    // 5. Requête HTTP avec flux asynchrone
    let response = client.get(source_url).send().await.map_err(|e| {
        emit_failure(app, download_id, 0, None);
        CommandError::NetworkError(format!("Erreur lors de la requête HTTP : {}", e))
    })?;

    let status = response.status();
    if !status.is_success() {
        emit_failure(app, download_id, 0, None);
        return Err(CommandError::DownloadFailed(format!(
            "Le serveur a renvoyé un statut HTTP {}",
            status
        )));
    }

    let total_bytes = response.content_length();

    // 6. Ouverture du fichier temporaire avec buffer non-bloquant de 128 Ko
    let file = tokio::fs::File::create(&part_file_path)
        .await
        .map_err(|e| {
            emit_failure(app, download_id, 0, total_bytes);
            CommandError::FileSystemError(format!(
                "Impossible de créer le fichier temporaire : {}",
                e
            ))
        })?;
    let mut writer = BufWriter::with_capacity(128 * 1024, file);

    let mut stream = response.bytes_stream();
    let mut downloaded_bytes: u64 = 0;

    let mut last_emit_instant = Instant::now();
    let mut last_emit_bytes: u64 = 0;
    let mut smoothed_speed: u64 = 0;

    // 7. Pipeline de réception et d'écriture des blocs
    while let Some(chunk_result) = stream.next().await {
        // Vérification d'un signal d'annulation
        if let Some(ref token) = cancel_token
            && token.load(Ordering::Relaxed)
        {
            drop(writer);
            let _ = tokio::fs::remove_file(&part_file_path).await;

            let cancel_payload = DownloadProgressPayload {
                download_id: download_id.to_string(),
                status: DownloadStatus::Cancelled,
                progress_percentage: 0.0,
                downloaded_bytes,
                total_bytes,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
            };
            let _ = app.emit("download-progress", &cancel_payload);

            return Err(CommandError::DownloadCancelled);
        }

        let chunk = match chunk_result {
            Ok(bytes) => bytes,
            Err(e) => {
                drop(writer);
                let _ = tokio::fs::remove_file(&part_file_path).await;
                emit_failure(app, download_id, downloaded_bytes, total_bytes);
                return Err(CommandError::NetworkError(format!(
                    "Interruption de flux réseau : {}",
                    e
                )));
            }
        };

        if let Err(e) = writer.write_all(&chunk).await {
            drop(writer);
            let _ = tokio::fs::remove_file(&part_file_path).await;
            emit_failure(app, download_id, downloaded_bytes, total_bytes);
            return Err(CommandError::FileSystemError(format!(
                "Erreur d'écriture disque : {}",
                e
            )));
        }

        downloaded_bytes += chunk.len() as u64;

        let now = Instant::now();
        let elapsed = now.duration_since(last_emit_instant);

        // Régulation d'émission : au maximum toutes les 100ms
        if elapsed >= Duration::from_millis(100) {
            let delta_bytes = downloaded_bytes.saturating_sub(last_emit_bytes);
            let instant_speed = if elapsed.as_secs_f64() > 0.0 {
                (delta_bytes as f64 / elapsed.as_secs_f64()) as u64
            } else {
                0
            };

            // Lissage de vitesse via moyenne mobile exponentielle (EMA)
            smoothed_speed = if smoothed_speed == 0 {
                instant_speed
            } else {
                ((0.35 * instant_speed as f64) + (0.65 * smoothed_speed as f64)) as u64
            };

            let (pct, eta) = if let Some(total) = total_bytes {
                let p = if total > 0 {
                    ((downloaded_bytes as f64 / total as f64) * 100.0).min(99.9)
                } else {
                    0.0
                };
                let remaining = total.saturating_sub(downloaded_bytes);
                let est_time = remaining.checked_div(smoothed_speed);
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
                speed_bytes_per_sec: smoothed_speed,
                eta_seconds: eta,
            };

            let _ = app.emit("download-progress", &payload);

            last_emit_instant = now;
            last_emit_bytes = downloaded_bytes;
        }
    }

    // 8. Synchronisation finale du tampon sur le support de stockage
    if let Err(e) = writer.flush().await {
        let _ = tokio::fs::remove_file(&part_file_path).await;
        return Err(CommandError::FileSystemError(format!(
            "Échec de synchronisation disque : {}",
            e
        )));
    }
    drop(writer);

    // 9. Renommage atomique du fichier temporaire vers sa destination définitive
    if let Err(e) = tokio::fs::rename(&part_file_path, target_file_path).await {
        let _ = tokio::fs::remove_file(&part_file_path).await;
        return Err(CommandError::FileSystemError(format!(
            "Impossible de finaliser le fichier téléchargé : {}",
            e
        )));
    }

    // 10. Émission de l'achèvement avec succès (100%)
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

fn emit_failure<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    downloaded: u64,
    total: Option<u64>,
) {
    let payload = DownloadProgressPayload {
        download_id: download_id.to_string(),
        status: DownloadStatus::Failed,
        progress_percentage: 0.0,
        downloaded_bytes: downloaded,
        total_bytes: total,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
    };
    let _ = app.emit("download-progress", &payload);
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

    #[test]
    fn test_download_registry_lifecycle() {
        let registry = DownloadRegistry::default();
        let id = "test_download_123";

        assert!(!registry.is_active(id));
        let token = registry.register(id);
        assert!(registry.is_active(id));
        assert!(!token.load(Ordering::Relaxed));

        let cancelled = registry.cancel(id);
        assert!(cancelled);
        assert!(token.load(Ordering::Relaxed));

        registry.unregister(id);
        assert!(!registry.is_active(id));
    }
}
