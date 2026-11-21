use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

use crate::error::CommandError;
use crate::extractor::{find_ffmpeg_binary, find_ytdlp_binary};

/// URLs officielles des composants précompilés.
const YTDLP_WINDOWS_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_UNIX_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";

#[allow(dead_code)]
const FFMPEG_WINDOWS_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

/// État global d'installation des dépendances tierces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub ytdlp_installed: bool,
    pub ytdlp_path: Option<String>,
    pub ytdlp_version: Option<String>,
    pub ffmpeg_installed: bool,
    pub ffmpeg_path: Option<String>,
    pub local_bin_dir: String,
    pub is_installing: bool,
}

/// Événement de progression émis vers le frontend via Tauri Event (`dependency-progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyProgressPayload {
    pub component: String, // "yt-dlp" | "ffmpeg"
    pub stage: String,     // "starting" | "downloading" | "extracting" | "ready" | "error"
    pub percentage: u8,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

/// Indicateur atomique évitant les téléchargements simultanés.
static IS_INSTALLING: AtomicBool = AtomicBool::new(false);

/// Renvoie le chemin du répertoire local dédié aux binaires tiers de ktk-download.
///
/// Windows : `%LOCALAPPDATA%\ktk-download\bin\`
/// Linux   : `~/.local/share/ktk-download/bin/`
/// macOS   : `~/Library/Application Support/ktk-download/bin/`
pub fn get_local_bin_dir() -> Result<PathBuf, CommandError> {
    let base = dirs::data_local_dir().ok_or_else(|| {
        CommandError::Internal("Impossible de déterminer le dossier de données local.".to_string())
    })?;
    let bin_dir = base.join("ktk-download").join("bin");
    if !bin_dir.exists() {
        std::fs::create_dir_all(&bin_dir).map_err(|e| {
            CommandError::Internal(format!(
                "Impossible de créer le dossier de stockage des binaires : {}",
                e
            ))
        })?;
    }
    Ok(bin_dir)
}

/// Vérifie l'état actuel des dépendances (`yt-dlp` et `ffmpeg`).
pub fn check_dependencies() -> DependencyStatus {
    let ytdlp_path = find_ytdlp_binary();
    let ffmpeg_path = find_ffmpeg_binary();

    let ytdlp_version = ytdlp_path.as_ref().and_then(|path| {
        std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    String::from_utf8(out.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    });

    let local_bin_dir = get_local_bin_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Inconnu".to_string());

    DependencyStatus {
        ytdlp_installed: ytdlp_path.is_some(),
        ytdlp_path: ytdlp_path.map(|p| p.to_string_lossy().to_string()),
        ytdlp_version,
        ffmpeg_installed: ffmpeg_path.is_some(),
        ffmpeg_path: ffmpeg_path.map(|p| p.to_string_lossy().to_string()),
        local_bin_dir,
        is_installing: IS_INSTALLING.load(Ordering::SeqCst),
    }
}

/// Télécharge un fichier HTTP(S) avec suivi de la progression et émission d'événements.
async fn download_file_with_progress(
    app: &AppHandle,
    client: &Client,
    url: &str,
    dest_path: &Path,
    component_name: &str,
) -> Result<(), CommandError> {
    let part_path = dest_path.with_extension("part");

    app.emit(
        "dependency-progress",
        DependencyProgressPayload {
            component: component_name.to_string(),
            stage: "starting".to_string(),
            percentage: 0,
            bytes_downloaded: 0,
            total_bytes: None,
            message: format!("Connexion à {}...", component_name),
        },
    )
    .ok();

    let response = client
        .get(url)
        .header("User-Agent", "ktk-download/0.1.0")
        .send()
        .await
        .map_err(|e| {
            CommandError::NetworkError(format!(
                "Échec de connexion pour le téléchargement de {} : {}",
                component_name, e
            ))
        })?;

    if !response.status().is_success() {
        return Err(CommandError::NetworkError(format!(
            "Réponse HTTP {} lors du téléchargement de {}.",
            response.status(),
            component_name
        )));
    }

    let total_bytes = response.content_length();
    let mut stream = response.bytes_stream();

    let mut out_file = File::create(&part_path).map_err(|e| {
        CommandError::Internal(format!(
            "Impossible de créer le fichier temporaire {:?} : {}",
            part_path, e
        ))
    })?;

    let mut downloaded: u64 = 0;
    let mut last_emit_percentage: u8 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| {
            CommandError::NetworkError(format!(
                "Interruption du flux pendant le téléchargement de {} : {}",
                component_name, e
            ))
        })?;

        out_file.write_all(&chunk).map_err(|e| {
            CommandError::Internal(format!("Erreur d'écriture sur le disque : {}", e))
        })?;

        downloaded += chunk.len() as u64;

        let percentage = total_bytes
            .and_then(|total| (downloaded * 100).checked_div(total))
            .map(|p| p.min(100) as u8)
            .unwrap_or(0);

        if percentage != last_emit_percentage || downloaded.is_multiple_of(512 * 1024) {
            last_emit_percentage = percentage;
            app.emit(
                "dependency-progress",
                DependencyProgressPayload {
                    component: component_name.to_string(),
                    stage: "downloading".to_string(),
                    percentage,
                    bytes_downloaded: downloaded,
                    total_bytes,
                    message: format!(
                        "Téléchargement de {} ({}%)...",
                        component_name, percentage
                    ),
                },
            )
            .ok();
        }
    }

    out_file.flush().map_err(|e| {
        CommandError::Internal(format!("Erreur de finalisation du fichier : {}", e))
    })?;
    drop(out_file);

    // Remplacement atomique du fichier temporaire par le fichier final
    if dest_path.exists() {
        let _ = std::fs::remove_file(dest_path);
    }
    std::fs::rename(&part_path, dest_path).map_err(|e| {
        CommandError::Internal(format!(
            "Impossible de renommer {:?} vers {:?} : {}",
            part_path, dest_path, e
        ))
    })?;

    // Attribution des droits d'exécution sous Unix (0o755)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(dest_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(dest_path, perms);
        }
    }

    Ok(())
}

/// Extrait les exécutables `ffmpeg.exe` et `ffprobe.exe` depuis une archive ZIP.
#[allow(dead_code)]
fn extract_ffmpeg_from_zip(zip_path: &Path, target_dir: &Path) -> Result<(), CommandError> {
    let file = File::open(zip_path).map_err(|e| {
        CommandError::ExtractionFailed(format!("Impossible d'ouvrir l'archive zip ffmpeg : {}", e))
    })?;

    let mut archive = ZipArchive::new(file).map_err(|e| {
        CommandError::ExtractionFailed(format!("Archive zip ffmpeg invalide : {}", e))
    })?;

    let mut extracted_count = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            CommandError::ExtractionFailed(format!("Erreur lecture élément zip : {}", e))
        })?;

        let entry_name = entry
            .name()
            .map_err(|e| {
                CommandError::ExtractionFailed(format!("Nom de fichier zip invalide : {}", e))
            })?
            .to_string();
        let lower = entry_name.to_lowercase();

        if lower.ends_with("ffmpeg.exe") || lower.ends_with("ffprobe.exe") {
            let file_name = Path::new(&entry_name).file_name().ok_or_else(|| {
                CommandError::ExtractionFailed("Nom de fichier invalide dans l'archive.".to_string())
            })?;

            let dest_path = target_dir.join(file_name);
            let mut out = File::create(&dest_path).map_err(|e| {
                CommandError::Internal(format!(
                    "Impossible de créer le binaire extrait {:?} : {}",
                    dest_path, e
                ))
            })?;

            io::copy(&mut entry, &mut out).map_err(|e| {
                CommandError::Internal(format!("Échec de décompression vers {:?} : {}", dest_path, e))
            })?;

            extracted_count += 1;
        }
    }

    if extracted_count == 0 {
        return Err(CommandError::ExtractionFailed(
            "Aucun binaire ffmpeg.exe trouvé dans l'archive téléchargée.".to_string(),
        ));
    }

    Ok(())
}

/// Installe ou met à jour les dépendances manquantes (`yt-dlp` et `ffmpeg`).
pub async fn install_dependencies(app: AppHandle) -> Result<DependencyStatus, CommandError> {
    if IS_INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(CommandError::Internal(
            "Une installation de dépendances est déjà en cours.".to_string(),
        ));
    }

    struct ResetGuard;
    impl Drop for ResetGuard {
        fn drop(&mut self) {
            IS_INSTALLING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = ResetGuard;

    let bin_dir = get_local_bin_dir()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| CommandError::Internal(format!("Échec initialisation HTTP : {}", e)))?;

    // 1. Installation de yt-dlp
    let ytdlp_target = if cfg!(windows) {
        bin_dir.join("yt-dlp.exe")
    } else {
        bin_dir.join("yt-dlp")
    };

    let ytdlp_url = if cfg!(windows) {
        YTDLP_WINDOWS_URL
    } else {
        YTDLP_UNIX_URL
    };

    log::info!(
        "Téléchargement automatique de yt-dlp vers {:?}",
        ytdlp_target
    );
    download_file_with_progress(&app, &client, ytdlp_url, &ytdlp_target, "yt-dlp").await?;

    app.emit(
        "dependency-progress",
        DependencyProgressPayload {
            component: "yt-dlp".to_string(),
            stage: "ready".to_string(),
            percentage: 100,
            bytes_downloaded: 0,
            total_bytes: None,
            message: "yt-dlp est prêt à l'emploi !".to_string(),
        },
    )
    .ok();

    // 2. Installation de ffmpeg sous Windows si absent
    #[cfg(windows)]
    {
        let ffmpeg_target = bin_dir.join("ffmpeg.exe");
        if !ffmpeg_target.exists() && find_ffmpeg_binary().is_none() {
            log::info!("Téléchargement automatique de FFmpeg pour Windows...");
            let temp_zip = bin_dir.join("ffmpeg-archive.zip");

            download_file_with_progress(&app, &client, FFMPEG_WINDOWS_URL, &temp_zip, "ffmpeg")
                .await?;

            app.emit(
                "dependency-progress",
                DependencyProgressPayload {
                    component: "ffmpeg".to_string(),
                    stage: "extracting".to_string(),
                    percentage: 95,
                    bytes_downloaded: 0,
                    total_bytes: None,
                    message: "Extraction de FFmpeg en cours...".to_string(),
                },
            )
            .ok();

            extract_ffmpeg_from_zip(&temp_zip, &bin_dir)?;
            let _ = std::fs::remove_file(&temp_zip);

            app.emit(
                "dependency-progress",
                DependencyProgressPayload {
                    component: "ffmpeg".to_string(),
                    stage: "ready".to_string(),
                    percentage: 100,
                    bytes_downloaded: 0,
                    total_bytes: None,
                    message: "FFmpeg est prêt !".to_string(),
                },
            )
            .ok();
        }
    }

    Ok(check_dependencies())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_bin_dir() {
        let bin_dir = get_local_bin_dir().expect("Le dossier local doit être résoluble");
        assert!(bin_dir.ends_with("bin"));
        assert!(bin_dir.exists());
    }

    #[test]
    fn test_check_dependencies_returns_valid_structure() {
        let status = check_dependencies();
        assert!(!status.local_bin_dir.is_empty());
        // En environnement de dev avec yt-dlp ou sans, la structure doit être valide
        if let Some(path) = status.ytdlp_path {
            assert!(status.ytdlp_installed);
            assert!(!path.is_empty());
        }
    }

    #[test]
    fn test_dependency_progress_serialization() {
        let payload = DependencyProgressPayload {
            component: "yt-dlp".to_string(),
            stage: "downloading".to_string(),
            percentage: 42,
            bytes_downloaded: 4200,
            total_bytes: Some(10000),
            message: "En cours...".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"percentage\":42"));
        assert!(json.contains("\"component\":\"yt-dlp\""));
    }
}

