use std::path::{Path, PathBuf};
use std::process::Stdio;

use regex::Regex;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::CommandError;
use crate::models::{DownloadProgressPayload, DownloadStatus, VideoFormat, VideoMetadata};

/// Localise le binaire `yt-dlp` sur le système hôte.
///
/// Recherche dans l'ordre :
/// 1. La variable d'environnement `YTDLP_PATH` si définie.
/// 2. Le répertoire utilisateur `$HOME/.local/bin/yt-dlp`.
/// 3. Les chemins système usuels (`/usr/local/bin/yt-dlp`, `/usr/bin/yt-dlp`).
/// 4. Le binaire `yt-dlp` accessible dans le `PATH`.
pub fn find_ytdlp_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("YTDLP_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let local_bin = home.join(".local/bin/yt-dlp");
        if local_bin.exists() {
            return Some(local_bin);
        }
    }

    let standard_paths = [
        "/usr/local/bin/yt-dlp",
        "/usr/bin/yt-dlp",
        "/opt/homebrew/bin/yt-dlp",
    ];

    for path_str in standard_paths {
        let p = PathBuf::from(path_str);
        if p.exists() {
            return Some(p);
        }
    }

    // Vérification de la disponibilité dans le PATH système
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("yt-dlp");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Extrait les métadonnées d'une vidéo en utilisant `yt-dlp` ou par inspection HTML de secours (ex: MoviBox).
pub async fn extract_metadata(url: &str) -> Result<VideoMetadata, CommandError> {
    let parsed_url = url::Url::parse(url)
        .map_err(|e| CommandError::InvalidUrl(format!("Format d'adresse invalide : {}", e)))?;

    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(CommandError::InvalidUrl(
            "Seuls les protocoles HTTP et HTTPS sont pris en charge.".to_string(),
        ));
    }

    let host = parsed_url.host_str().unwrap_or("").to_lowercase();

    // 1. Tentative d'extraction avec yt-dlp si présent
    if let Some(ytdlp_bin) = find_ytdlp_binary()
        && let Ok(metadata) = extract_with_ytdlp(&ytdlp_bin, url).await
    {
        return Ok(metadata);
    }

    // 2. Extraction de secours : scraping direct pour MoviBox et lecteurs web HTML5/HLS
    if let Ok(metadata) = extract_from_webpage(url).await {
        return Ok(metadata);
    }

    // 3. Détection heuristique de secours (ex: mode hors-ligne ou CDN restreint)
    let platform = if host.contains("youtube.com") || host.contains("youtu.be") {
        "YouTube".to_string()
    } else if host.contains("movibox.net") || host.contains("movibox") {
        "MoviBox".to_string()
    } else {
        "Flux Web / Direct".to_string()
    };

    Ok(fallback_metadata(url, &host, &platform))
}

fn fallback_metadata(url: &str, host: &str, platform: &str) -> VideoMetadata {
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

    VideoMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        title: format!("Vidéo {}", platform),
        description: Some(format!("Flux analysé depuis {}", host)),
        author: None,
        thumbnail_url: None,
        duration_seconds: None,
        source_platform: platform.to_string(),
        source_url: url.to_string(),
        available_formats: formats,
    }
}

/// Exécute `yt-dlp --dump-single-json` et parse le JSON retourné.
async fn extract_with_ytdlp(ytdlp_bin: &Path, url: &str) -> Result<VideoMetadata, CommandError> {
    let output = tokio::process::Command::new(ytdlp_bin)
        .arg("--dump-single-json")
        .arg("--no-warnings")
        .arg("--no-playlist")
        .arg(url)
        .output()
        .await
        .map_err(|e| {
            CommandError::ExtractionFailed(format!("Échec d'exécution de yt-dlp : {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CommandError::ExtractionFailed(format!(
            "yt-dlp a échoué : {}",
            stderr.trim()
        )));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&json_str)
        .map_err(|e| CommandError::ExtractionFailed(format!("Réponse JSON invalide : {}", e)))?;

    let id = parsed["id"].as_str().unwrap_or("unknown").to_string();
    let title = parsed["title"]
        .as_str()
        .unwrap_or("Vidéo sans titre")
        .to_string();
    let description = parsed["description"].as_str().map(|s| s.to_string());
    let author = parsed["uploader"]
        .as_str()
        .or_else(|| parsed["channel"].as_str())
        .map(|s| s.to_string());
    let thumbnail_url = parsed["thumbnail"].as_str().map(|s| s.to_string());
    let duration_seconds = parsed["duration"].as_u64();
    let raw_extractor = parsed["extractor_key"]
        .as_str()
        .or_else(|| parsed["extractor"].as_str())
        .unwrap_or("Web");

    let extractor_name = if raw_extractor.to_lowercase().contains("youtube") {
        "YouTube".to_string()
    } else if raw_extractor.to_lowercase().contains("movibox") {
        "MoviBox".to_string()
    } else {
        raw_extractor.to_string()
    };

    let mut formats = Vec::new();

    // Format "Meilleure qualité (Auto)" par défaut
    formats.push(VideoFormat {
        format_id: "best".to_string(),
        label: "Meilleure qualité disponible (Auto)".to_string(),
        resolution: Some("Auto".to_string()),
        extension: "mp4".to_string(),
        file_size_bytes: None,
        has_video: true,
        has_audio: true,
        direct_url: None,
    });

    // Formats additionnels si présents dans le JSON
    if let Some(raw_formats) = parsed["formats"].as_array() {
        let mut added_resolutions = std::collections::HashSet::new();

        for f in raw_formats.iter().rev() {
            let has_video = f["vcodec"].as_str().map(|v| v != "none").unwrap_or(false);
            let has_audio = f["acodec"].as_str().map(|a| a != "none").unwrap_or(false);

            if has_video && has_audio {
                let height = f["height"].as_u64().unwrap_or(0);
                if height >= 360 && added_resolutions.insert(height) {
                    let fmt_id = f["format_id"].as_str().unwrap_or("").to_string();
                    let ext = f["ext"].as_str().unwrap_or("mp4").to_string();
                    let direct = f["url"].as_str().map(|s| s.to_string());

                    formats.push(VideoFormat {
                        format_id: fmt_id,
                        label: format!("{}p", height),
                        resolution: Some(format!("{}p", height)),
                        extension: ext,
                        file_size_bytes: f["filesize"]
                            .as_u64()
                            .or_else(|| f["filesize_approx"].as_u64()),
                        has_video: true,
                        has_audio: true,
                        direct_url: direct,
                    });
                }
            }
        }
    }

    // Format audio uniquement
    formats.push(VideoFormat {
        format_id: "audio_only".to_string(),
        label: "Audio uniquement (MP3)".to_string(),
        resolution: None,
        extension: "mp3".to_string(),
        file_size_bytes: None,
        has_video: false,
        has_audio: true,
        direct_url: None,
    });

    Ok(VideoMetadata {
        id,
        title,
        description,
        author,
        thumbnail_url,
        duration_seconds,
        source_platform: extractor_name,
        source_url: url.to_string(),
        available_formats: formats,
    })
}

/// Extraction de secours par analyse directe du code HTML de la page (ex: MoviBox, balises video, HLS .m3u8).
async fn extract_from_webpage(url: &str) -> Result<VideoMetadata, CommandError> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| CommandError::NetworkError(e.to_string()))?;

    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| CommandError::NetworkError(e.to_string()))?
        .text()
        .await
        .map_err(|e| CommandError::ExtractionFailed(e.to_string()))?;

    // Extraction du titre de la page
    let title_regex = Regex::new(r"(?i)<title[^>]*>([^<]+)</title>").unwrap();
    let title = title_regex
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "Vidéo Web".to_string());

    // Recherche de flux directs MP4 ou HLS (.m3u8)
    let stream_regex = Regex::new(r#"(https?://[^\s"'<>]+\.(?:m3u8|mp4)[^\s"'<>]*)"#).unwrap();
    let direct_stream_url = stream_regex
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let platform = if url.contains("movibox") {
        "MoviBox".to_string()
    } else {
        "Lecteur Web Direct".to_string()
    };

    let formats = vec![
        VideoFormat {
            format_id: "best".to_string(),
            label: "Flux original (Meilleure qualité)".to_string(),
            resolution: Some("Auto".to_string()),
            extension: "mp4".to_string(),
            file_size_bytes: None,
            has_video: true,
            has_audio: true,
            direct_url: direct_stream_url.clone(),
        },
        VideoFormat {
            format_id: "audio_only".to_string(),
            label: "Audio uniquement (MP3)".to_string(),
            resolution: None,
            extension: "mp3".to_string(),
            file_size_bytes: None,
            has_video: false,
            has_audio: true,
            direct_url: direct_stream_url,
        },
    ];

    Ok(VideoMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        description: Some(format!("Source analysée depuis {}", platform)),
        author: None,
        thumbnail_url: None,
        duration_seconds: None,
        source_platform: platform,
        source_url: url.to_string(),
        available_formats: formats,
    })
}

/// Télécharge une vidéo via `yt-dlp` en suivant sa progression ligne par ligne en temps réel.
pub async fn download_with_ytdlp<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    url: &str,
    format_id: Option<&str>,
    target_path: &Path,
) -> Result<u64, CommandError> {
    let ytdlp_bin = find_ytdlp_binary().ok_or_else(|| {
        CommandError::Internal("Le binaire yt-dlp est introuvable sur le système.".to_string())
    })?;

    // Assure la création du dossier parent
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CommandError::FileSystemError(e.to_string()))?;
    }

    let mut cmd = tokio::process::Command::new(&ytdlp_bin);
    cmd.arg("--newline")
        .arg("--progress")
        .arg("--progress-template")
        .arg("ktk:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress._total_bytes_str)s")
        .arg("-o")
        .arg(target_path)
        .arg(url);

    if let Some(fmt) = format_id {
        if fmt == "audio_only" {
            cmd.arg("-x").arg("--audio-format").arg("mp3");
        } else if fmt != "best" && !fmt.is_empty() {
            cmd.arg("-f").arg(fmt);
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CommandError::Internal(format!("Échec de lancement de yt-dlp : {}", e)))?;

    let stdout = child.stdout.take().ok_or_else(|| {
        CommandError::Internal("Impossible de capturer la sortie standard".to_string())
    })?;

    let mut reader = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        if let Some(progress_data) = line.strip_prefix("ktk:") {
            let parts: Vec<&str> = progress_data.split('|').collect();
            if parts.len() >= 4 {
                let pct = parse_percent_str(parts[0]);
                let speed = parse_speed_str(parts[1]);
                let eta = parse_eta_str(parts[2]);
                let total = parse_bytes_str(parts[3]);

                let downloaded = if let Some(tot) = total {
                    ((pct / 100.0) * (tot as f64)) as u64
                } else {
                    0
                };

                let payload = DownloadProgressPayload {
                    download_id: download_id.to_string(),
                    status: DownloadStatus::Downloading,
                    progress_percentage: pct,
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                    speed_bytes_per_sec: speed,
                    eta_seconds: eta,
                };

                let _ = app.emit("download-progress", &payload);
            }
        }
    }

    let status = child.wait().await.map_err(|e| {
        CommandError::Internal(format!("Erreur lors de l'attente du processus : {}", e))
    })?;

    if !status.success() {
        return Err(CommandError::DownloadFailed(
            "Le téléchargement via yt-dlp s'est interrompu anormalement.".to_string(),
        ));
    }

    // Récupération de la taille finale du fichier
    let metadata = tokio::fs::metadata(target_path)
        .await
        .map_err(|e| CommandError::FileSystemError(e.to_string()))?;

    let final_bytes = metadata.len();

    let completed_payload = DownloadProgressPayload {
        download_id: download_id.to_string(),
        status: DownloadStatus::Completed,
        progress_percentage: 100.0,
        downloaded_bytes: final_bytes,
        total_bytes: Some(final_bytes),
        speed_bytes_per_sec: 0,
        eta_seconds: Some(0),
    };
    let _ = app.emit("download-progress", &completed_payload);

    Ok(final_bytes)
}

/// Convertit une chaîne de pourcentage (ex: "  45.2%") en nombre à virgule flottante.
pub fn parse_percent_str(s: &str) -> f64 {
    let clean = s.replace('%', "").trim().to_string();
    clean.parse::<f64>().unwrap_or(0.0)
}

/// Convertit une vitesse textuelle (ex: " 2.50MiB/s", " 450.00KiB/s") en octets par seconde.
pub fn parse_speed_str(s: &str) -> u64 {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "N/A" {
        return 0;
    }

    if let Some(val_str) = trimmed.strip_suffix("GiB/s") {
        (val_str.trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0 * 1024.0) as u64
    } else if let Some(val_str) = trimmed.strip_suffix("MiB/s") {
        (val_str.trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0) as u64
    } else if let Some(val_str) = trimmed.strip_suffix("KiB/s") {
        (val_str.trim().parse::<f64>().unwrap_or(0.0) * 1024.0) as u64
    } else if let Some(val_str) = trimmed.strip_suffix("B/s") {
        val_str.trim().parse::<f64>().unwrap_or(0.0) as u64
    } else {
        0
    }
}

/// Convertit un temps estimé restant (ex: "01:23", "00:15", "01:10:05") en secondes.
pub fn parse_eta_str(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "N/A" {
        return None;
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.len() {
        2 => {
            let m = parts[0].parse::<u64>().ok()?;
            let sec = parts[1].parse::<u64>().ok()?;
            Some(m * 60 + sec)
        }
        3 => {
            let h = parts[0].parse::<u64>().ok()?;
            let m = parts[1].parse::<u64>().ok()?;
            let sec = parts[2].parse::<u64>().ok()?;
            Some(h * 3600 + m * 60 + sec)
        }
        _ => None,
    }
}

/// Convertit une taille de fichier textuelle (ex: " 15.40MiB", " 1.20GiB") en octets.
pub fn parse_bytes_str(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "N/A" {
        return None;
    }

    if let Some(val_str) = trimmed.strip_suffix("GiB") {
        Some((val_str.trim().parse::<f64>().ok()? * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if let Some(val_str) = trimmed.strip_suffix("MiB") {
        Some((val_str.trim().parse::<f64>().ok()? * 1024.0 * 1024.0) as u64)
    } else if let Some(val_str) = trimmed.strip_suffix("KiB") {
        Some((val_str.trim().parse::<f64>().ok()? * 1024.0) as u64)
    } else if let Some(val_str) = trimmed.strip_suffix("B") {
        val_str.trim().parse::<u64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_percent() {
        assert_eq!(parse_percent_str(" 45.2%"), 45.2);
        assert_eq!(parse_percent_str("100.0%"), 100.0);
        assert_eq!(parse_percent_str("0.0%"), 0.0);
    }

    #[test]
    fn test_parse_speed() {
        assert_eq!(parse_speed_str(" 2.0MiB/s"), 2 * 1024 * 1024);
        assert_eq!(parse_speed_str(" 500KiB/s"), 500 * 1024);
        assert_eq!(parse_speed_str("N/A"), 0);
    }

    #[test]
    fn test_parse_eta() {
        assert_eq!(parse_eta_str("01:30"), Some(90));
        assert_eq!(parse_eta_str("01:00:15"), Some(3615));
        assert_eq!(parse_eta_str("N/A"), None);
    }

    #[test]
    fn test_parse_bytes() {
        assert_eq!(parse_bytes_str(" 10.0MiB"), Some(10 * 1024 * 1024));
        assert_eq!(parse_bytes_str(" 1.0GiB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_bytes_str("N/A"), None);
    }
}
