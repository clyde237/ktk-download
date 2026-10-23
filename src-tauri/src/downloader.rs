use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
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

/// Extensions des fichiers média produits ou reconnus par l'application.
pub const KNOWN_MEDIA_EXTENSIONS: &[&str] = &["mp4", "mp3", "m4a", "webm", "mkv", "mov"];

/// Taille maximale du nom de base, en octets : laisse la place aux suffixes de yt-dlp
/// (`.f251-drc.webm.part`) sous la limite de 255 octets des systèmes de fichiers courants.
const MAX_FILE_STEM_BYTES: usize = 150;

/// Noms réservés par Windows, interdits comme nom de fichier quelle que soit l'extension.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Résout et sécurise le chemin de destination pour l'enregistrement d'un fichier téléchargé.
///
/// Le nom demandé (en pratique le titre de la vidéo) est assaini pour être valide sur tous les
/// systèmes et ne permettre aucune traversée de répertoire. Un fichier existant n'est jamais
/// écrasé ni réutilisé : le nom reçoit un suffixe « (2) », « (3) »… Sans nom exploitable,
/// `video_<identifiant>` est utilisé. Sans dossier, le dossier `Downloads` du système.
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

    let (stem, extension) = custom_filename
        .map(sanitize_file_name)
        .and_then(|name| split_file_name(&name, default_extension))
        .unwrap_or_else(|| {
            (
                format!("video_{}", fallback_id),
                default_extension.to_string(),
            )
        });

    unique_output_path(&base_dir, &stem, &extension, fallback_id)
}

/// Remplace les caractères interdits par les systèmes de fichiers et normalise les espaces.
fn sanitize_file_name(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => ' ',
            other => other,
        })
        .collect();
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sépare nom de base et extension. Une extension média connue est conservée ; sinon le nom
/// complet sert de base (un titre peut contenir des points) et l'extension par défaut s'ajoute.
fn split_file_name(name: &str, default_extension: &str) -> Option<(String, String)> {
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext))
            if KNOWN_MEDIA_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) =>
        {
            (stem, ext.to_ascii_lowercase())
        }
        _ => (name, default_extension.to_string()),
    };

    // Points et espaces en bord de nom : fichier caché sous Unix, nom invalide sous Windows,
    // et « .. » ne doit jamais subsister.
    let is_edge = |c: char| c == '.' || c == ' ';
    let stem = truncate_to_bytes(stem.trim_matches(is_edge), MAX_FILE_STEM_BYTES)
        .trim_end_matches(is_edge);
    if stem.is_empty() {
        return None;
    }

    let base_name = stem.split('.').next().unwrap_or(stem).to_ascii_uppercase();
    let stem = if WINDOWS_RESERVED_NAMES.contains(&base_name.as_str()) {
        format!("_{}", stem)
    } else {
        stem.to_string()
    };
    Some((stem, extension))
}

fn truncate_to_bytes(text: &str, max_bytes: usize) -> &str {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Choisit le premier nom libre : « Titre », « Titre (2) », « Titre (3) »…
///
/// yt-dlp choisit lui-même l'extension finale : un nom est pris dès qu'un fichier média connu
/// (ou son fichier `.part`) porte ce nom de base.
fn unique_output_path(dir: &Path, stem: &str, extension: &str, fallback_id: &str) -> PathBuf {
    let is_taken = |candidate: &str| {
        KNOWN_MEDIA_EXTENSIONS
            .iter()
            .copied()
            .chain(std::iter::once(extension))
            .any(|ext| {
                dir.join(format!("{}.{}", candidate, ext)).exists()
                    || dir.join(format!("{}.{}.part", candidate, ext)).exists()
            })
    };

    (1..=99)
        .map(|n| {
            if n == 1 {
                stem.to_string()
            } else {
                format!("{} ({})", stem, n)
            }
        })
        .find(|candidate| !is_taken(candidate))
        .map(|candidate| dir.join(format!("{}.{}", candidate, extension)))
        .unwrap_or_else(|| dir.join(format!("{} ({}).{}", stem, fallback_id, extension)))
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
        CommandError::from(e)
    })?;

    let status = response.status();
    if !status.is_success() {
        emit_failure(app, download_id, 0, None);
        return Err(CommandError::from_http_status(status));
    }

    // Une page web enregistrée sous `.mp4` passerait pour un téléchargement réussi.
    if let Some(content_type) = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        && is_non_media_content_type(content_type)
    {
        emit_failure(app, download_id, 0, None);
        return Err(CommandError::UnsupportedPlatform(
            "L'adresse mène à une page web ou à une liste de lecture, pas à un fichier vidéo téléchargeable directement.".to_string(),
        ));
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
                return Err(CommandError::from(e));
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

/// Types de contenu qui ne sont jamais le fichier vidéo lui-même : page web, réponse d'API, ou
/// manifeste de streaming (HLS/DASH) qu'il faudrait interpréter au lieu de l'enregistrer tel quel.
const NON_MEDIA_CONTENT_TYPES: &[&str] = &[
    "text/html",
    "application/xhtml+xml",
    "application/json",
    "application/vnd.apple.mpegurl",
    "application/x-mpegurl",
    "audio/mpegurl",
    "audio/x-mpegurl",
    "application/dash+xml",
];

fn is_non_media_content_type(content_type: &str) -> bool {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    NON_MEDIA_CONTENT_TYPES.contains(&mime.as_str())
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
    fn test_resolve_output_path_keeps_dotted_titles() {
        let path = resolve_output_path(
            Some("/tmp/ktk-inexistant"),
            Some("Mr. Bean: The Movie"),
            "abcdef12",
            "mp4",
        );
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "Mr. Bean_ The Movie.mp4"
        );
    }

    #[test]
    fn test_resolve_output_path_rejects_traversal_and_reserved_names() {
        let traversal = resolve_output_path(
            Some("/tmp/ktk-inexistant"),
            Some("../../etc/passwd"),
            "abcdef12",
            "mp4",
        );
        assert_eq!(
            traversal.parent().unwrap(),
            Path::new("/tmp/ktk-inexistant")
        );
        assert_eq!(
            traversal.file_name().unwrap().to_str().unwrap(),
            "_.._etc_passwd.mp4"
        );

        let reserved =
            resolve_output_path(Some("/tmp/ktk-inexistant"), Some("con"), "abcdef12", "mp4");
        assert_eq!(reserved.file_name().unwrap().to_str().unwrap(), "_con.mp4");

        let only_dots =
            resolve_output_path(Some("/tmp/ktk-inexistant"), Some(" .. "), "abcdef12", "mp4");
        assert_eq!(
            only_dots.file_name().unwrap().to_str().unwrap(),
            "video_abcdef12.mp4"
        );
    }

    #[test]
    fn test_resolve_output_path_normalizes_whitespace_and_length() {
        let path = resolve_output_path(
            Some("/tmp/ktk-inexistant"),
            Some("Clip\n\tfinal   cut"),
            "abcdef12",
            "mp4",
        );
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "Clip final cut.mp4"
        );

        // Titre multi-octets très long : coupé sur une frontière de caractère
        let long_title = "動画".repeat(200);
        let path = resolve_output_path(
            Some("/tmp/ktk-inexistant"),
            Some(&long_title),
            "abcdef12",
            "mp4",
        );
        let stem = path.file_stem().unwrap().to_str().unwrap();
        assert!(stem.len() <= MAX_FILE_STEM_BYTES);
        assert!(stem.chars().all(|c| c == '動' || c == '画'));
    }

    #[test]
    fn test_resolve_output_path_never_reuses_existing_file() {
        let dir = std::env::temp_dir().join("ktk-unique-name-test");
        std::fs::create_dir_all(&dir).unwrap();
        // yt-dlp a déjà produit « Clip.webm » : un nouveau « Clip » ne doit pas le réutiliser
        std::fs::write(dir.join("Clip.webm"), b"").unwrap();
        std::fs::write(dir.join("Clip (2).mp4.part"), b"").unwrap();

        let path = resolve_output_path(dir.to_str(), Some("Clip"), "abcdef12", "mp4");
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "Clip (3).mp4");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_output_path_extension_addition() {
        let path = resolve_output_path(Some("/tmp"), Some("my_vacation"), "abcdef12", "mp4");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "my_vacation.mp4");
    }

    #[test]
    fn test_is_non_media_content_type() {
        assert!(is_non_media_content_type("text/html; charset=utf-8"));
        assert!(is_non_media_content_type("TEXT/HTML"));
        assert!(is_non_media_content_type("application/vnd.apple.mpegurl"));

        assert!(!is_non_media_content_type("video/mp4"));
        assert!(!is_non_media_content_type("audio/mpeg"));
        assert!(!is_non_media_content_type("application/octet-stream"));
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
