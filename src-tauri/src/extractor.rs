use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::downloader::{self, DownloadControl, KNOWN_MEDIA_EXTENSIONS, StopRequest};
use crate::error::CommandError;
use crate::models::{DownloadProgressPayload, DownloadStatus, VideoFormat, VideoMetadata};
use crate::quality::{self, QualityPreset};

/// Fréquence maximale de vérification du jeton d'annulation pendant un téléchargement yt-dlp.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(250);

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

/// Extrait les métadonnées d'une vidéo avec `yt-dlp`, puis par analyse directe de la page
/// quand `allow_webpage_scan` l'autorise (fichiers et lecteurs web génériques uniquement).
///
/// Aucune métadonnée n'est inventée : si aucune vidéo n'est trouvée, une erreur est renvoyée,
/// pour ne jamais présenter comme analysée une vidéo impossible à télécharger.
pub async fn extract_metadata(
    url: &str,
    allow_webpage_scan: bool,
) -> Result<VideoMetadata, CommandError> {
    let parsed_url = url::Url::parse(url)
        .map_err(|e| CommandError::InvalidUrl(format!("Format d'adresse invalide : {}", e)))?;

    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(CommandError::InvalidUrl(
            "Seuls les protocoles HTTP et HTTPS sont pris en charge.".to_string(),
        ));
    }

    // 1. Extraction avec yt-dlp si présent
    let ytdlp_error = match find_ytdlp_binary() {
        Some(ytdlp_bin) => match extract_with_ytdlp(&ytdlp_bin, url).await {
            Ok(metadata) => return Ok(metadata),
            Err(e) => Some(e),
        },
        None => None,
    };

    // 2. Analyse directe de la réponse HTTP (fichier média ou lecteur HTML5/HLS)
    if allow_webpage_scan {
        return match extract_from_webpage(url).await {
            Ok(metadata) => Ok(metadata),
            // Le diagnostic de yt-dlp, quand il existe, est plus précis que celui du scan HTML.
            Err(e) => Err(ytdlp_error.unwrap_or(e)),
        };
    }

    Err(ytdlp_error.unwrap_or_else(|| {
        CommandError::ExtractionFailed(
            "yt-dlp est requis pour analyser cette plateforme, mais il est introuvable sur le système.".to_string(),
        )
    }))
}

/// Exécute `yt-dlp --dump-single-json` et parse le JSON retourné.
pub async fn extract_with_ytdlp(
    ytdlp_bin: &Path,
    url: &str,
) -> Result<VideoMetadata, CommandError> {
    let output = ytdlp_command(ytdlp_bin)
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
        return Err(CommandError::from_ytdlp_stderr(&stderr));
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
    } else {
        raw_extractor.to_string()
    };

    // Certains extracteurs décrivent un format unique à la racine du JSON, sans liste `formats`.
    let formats = match parsed["formats"].as_array() {
        Some(raw_formats) => quality::quality_options(raw_formats),
        None => quality::quality_options(std::slice::from_ref(&parsed)),
    };

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

/// Nom de plateforme des sources web génériques, identique à celui du validateur.
const GENERIC_WEB_PLATFORM: &str = "Flux Web / Direct";

/// Analyse de secours sans yt-dlp : reconnaît un fichier média servi directement, ou cherche dans
/// le code HTML de la page un lien vers un flux MP4 ou HLS (.m3u8).
async fn extract_from_webpage(url: &str) -> Result<VideoMetadata, CommandError> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| CommandError::NetworkError(e.to_string()))?;

    let response = client.get(url).send().await.map_err(CommandError::from)?;

    let status = response.status();
    if !status.is_success() {
        return Err(CommandError::from_http_status(status));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    // Fichier média servi directement : son contenu n'est jamais chargé en mémoire pour l'analyse.
    if is_direct_media(&content_type, response.url().path()) {
        let title = media_title_from_path(response.url().path());
        let file_size_bytes = response.content_length();
        return Ok(web_stream_metadata(url, title, url, file_size_bytes));
    }

    let html = response
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
    let Some(direct_stream_url) = stream_regex
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
    else {
        return Err(CommandError::ExtractionFailed(
            "Aucune vidéo téléchargeable n'a été détectée sur cette page.".to_string(),
        ));
    };

    Ok(web_stream_metadata(url, title, &direct_stream_url, None))
}

/// Une réponse est un fichier média si son type l'annonce, ou si un type binaire générique
/// accompagne une URL à l'extension média connue (cas fréquent des CDN).
fn is_direct_media(content_type: &str, url_path: &str) -> bool {
    let mime = content_type.split(';').next().unwrap_or_default().trim();
    if mime.starts_with("video/") || mime.starts_with("audio/") {
        return true;
    }

    let has_media_extension = Path::new(url_path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| KNOWN_MEDIA_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
    mime == "application/octet-stream" && has_media_extension
}

fn media_title_from_path(url_path: &str) -> String {
    Path::new(url_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Vidéo Web".to_string())
}

fn web_stream_metadata(
    source_url: &str,
    title: String,
    stream_url: &str,
    file_size_bytes: Option<u64>,
) -> VideoMetadata {
    // Sans yt-dlp, le fichier est enregistré tel quel : ni choix de qualité ni conversion MP3.
    let formats = vec![VideoFormat {
        format_id: QualityPreset::Best.id().to_string(),
        label: "Fichier original".to_string(),
        resolution: None,
        extension: "mp4".to_string(),
        file_size_bytes,
        has_video: true,
        has_audio: true,
        direct_url: Some(stream_url.to_string()),
    }];

    VideoMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        description: Some(format!("Source analysée depuis {}", GENERIC_WEB_PLATFORM)),
        author: None,
        thumbnail_url: None,
        duration_seconds: None,
        source_platform: GENERIC_WEB_PLATFORM.to_string(),
        source_url: source_url.to_string(),
        available_formats: formats,
    }
}

/// Télécharge une vidéo via `yt-dlp` en suivant sa progression ligne par ligne en temps réel.
///
/// Renvoie le chemin réel du fichier final et sa taille : yt-dlp choisit lui-même le conteneur
/// (`.mp4`, `.webm`, `.mkv`…), le nom effectif peut donc différer de `target_path`.
pub async fn download_with_ytdlp<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    url: &str,
    quality: QualityPreset,
    target_path: &Path,
    control: Option<Arc<DownloadControl>>,
) -> Result<(PathBuf, u64), CommandError> {
    let ytdlp_bin = find_ytdlp_binary().ok_or_else(|| {
        CommandError::Internal("Le binaire yt-dlp est introuvable sur le système.".to_string())
    })?;

    download_with_ytdlp_binary(
        app,
        download_id,
        url,
        quality,
        target_path,
        control,
        &ytdlp_bin,
    )
    .await
}

/// Télécharge une vidéo via un binaire `yt-dlp` spécifié en suivant sa progression ligne par ligne.
pub async fn download_with_ytdlp_binary<R: Runtime>(
    app: &AppHandle<R>,
    download_id: &str,
    url: &str,
    quality: QualityPreset,
    target_path: &Path,
    control: Option<Arc<DownloadControl>>,
    ytdlp_bin: &Path,
) -> Result<(PathBuf, u64), CommandError> {
    let output = YtdlpOutput::for_target(target_path).ok_or_else(|| {
        CommandError::FileSystemError("Chemin de destination invalide.".to_string())
    })?;

    // Assure la création du dossier parent
    tokio::fs::create_dir_all(&output.dir)
        .await
        .map_err(|e| CommandError::FileSystemError(e.to_string()))?;

    let ffmpeg_bin = find_ffmpeg_binary();
    if quality.is_audio_only() && ffmpeg_bin.is_none() {
        return Err(CommandError::DownloadFailed(
            "La conversion en MP3 nécessite ffmpeg, introuvable sur le système. Installez ffmpeg puis réessayez.".to_string(),
        ));
    }
    // Sans ffmpeg, yt-dlp télécharge audio et vidéo séparément sans pouvoir les fusionner :
    // on exige alors un format unique contenant déjà les deux flux.
    let requires_single_file = !quality.is_audio_only() && ffmpeg_bin.is_none();

    let mut cmd = ytdlp_command(ytdlp_bin);
    cmd.arg("--newline")
        .arg("--no-playlist")
        .arg("--progress")
        .arg("--progress-template")
        .arg("ktk:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress._total_bytes_str)s")
        .arg("--print")
        .arg(format!("after_move:{}%(filepath)s", FINAL_PATH_PREFIX))
        .arg("-o")
        .arg(output.template())
        .arg(url);

    // Même binaire ffmpeg pour la détection ci-dessus et pour yt-dlp
    if let Some(ffmpeg) = &ffmpeg_bin {
        cmd.arg("--ffmpeg-location").arg(ffmpeg);
    }

    cmd.args(quality.ytdlp_args(ffmpeg_bin.is_some()));

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CommandError::Internal(format!("Échec de lancement de yt-dlp : {}", e)))?;

    let stdout = child.stdout.take().ok_or_else(|| {
        CommandError::Internal("Impossible de capturer la sortie standard".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        CommandError::Internal("Impossible de capturer la sortie d'erreur".to_string())
    })?;

    // Lecture continue de stderr : un tampon plein bloquerait yt-dlp, et son contenu explique
    // les échecs à l'utilisateur.
    let stderr_task = tauri::async_runtime::spawn(collect_stderr_tail(stderr));

    let mut reader = BufReader::new(stdout).lines();
    let mut final_path: Option<PathBuf> = None;

    loop {
        // yt-dlp peut rester silencieux longtemps (connexion lente, fusion ffmpeg) : le délai
        // garantit que le jeton d'annulation est relu même sans nouvelle ligne de progression.
        let line = tokio::select! {
            next = reader.next_line() => match next {
                Ok(Some(line)) => Some(line),
                _ => break,
            },
            _ = tokio::time::sleep(CANCEL_POLL_INTERVAL) => None,
        };

        match control.as_ref().and_then(|c| c.stop_request()) {
            Some(StopRequest::Cancel) => {
                let _ = child.kill().await;
                output.remove_intermediate_files().await;
                downloader::emit_stopped(app, download_id, DownloadStatus::Cancelled, 0, None);
                return Err(CommandError::DownloadCancelled);
            }
            Some(StopRequest::Pause) => {
                // Les fichiers `.part` sont conservés : relancé avec le même nom, yt-dlp reprend
                // là où il s'était arrêté.
                let _ = child.kill().await;
                downloader::emit_stopped(app, download_id, DownloadStatus::Paused, 0, None);
                return Err(CommandError::DownloadPaused);
            }
            None => {}
        }

        let Some(line) = line else {
            continue;
        };

        if let Some(path) = line.strip_prefix(FINAL_PATH_PREFIX) {
            final_path = Some(PathBuf::from(path.trim()));
        } else if let Some(progress_data) = line.strip_prefix("ktk:") {
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
    let stderr_tail = stderr_task.await.unwrap_or_default();

    if !status.success() {
        output.remove_intermediate_files().await;
        return Err(ytdlp_failure_error(&stderr_tail, requires_single_file));
    }

    // yt-dlp peut réussir sans produire de fichier final (flux séparés non fusionnés) :
    // seul un fichier réellement présent sur le disque compte comme un succès.
    let final_file = match final_path {
        Some(path) => match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => Some((path, metadata.len())),
            _ => None,
        },
        None => None,
    };

    let Some((file_path, final_bytes)) = final_file else {
        output.remove_intermediate_files().await;
        return Err(CommandError::DownloadFailed(
            "yt-dlp n'a produit aucun fichier final : les flux audio et vidéo n'ont pas pu être fusionnés. Installez ffmpeg puis réessayez.".to_string(),
        ));
    };

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

    Ok((file_path, final_bytes))
}

/// Préfixe de la ligne `--print after_move:…` qui donne le chemin du fichier final.
const FINAL_PATH_PREFIX: &str = "ktk-file:";

/// Nombre de lignes de stderr conservées pour expliquer un échec.
const STDERR_TAIL_LINES: usize = 50;

/// Emplacement de sortie yt-dlp dérivé du chemin cible : même dossier et même nom de base,
/// extension laissée à yt-dlp (`%(ext)s`), seul à connaître le conteneur réellement produit.
struct YtdlpOutput {
    dir: PathBuf,
    base_name: String,
}

impl YtdlpOutput {
    fn for_target(target_path: &Path) -> Option<Self> {
        let dir = target_path.parent()?.to_path_buf();
        let has_known_extension = target_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| KNOWN_MEDIA_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
        let base_name = if has_known_extension {
            target_path.file_stem()?.to_str()?
        } else {
            target_path.file_name()?.to_str()?
        };
        Some(Self {
            dir,
            base_name: base_name.to_string(),
        })
    }

    /// Modèle `-o` pour yt-dlp. `%` y est un caractère spécial : il est doublé dans le chemin
    /// littéral pour qu'un dossier ou un nom contenant `%` ne soit pas interprété.
    fn template(&self) -> String {
        let literal = self
            .dir
            .join(&self.base_name)
            .to_string_lossy()
            .replace('%', "%%");
        format!("{}.%(ext)s", literal)
    }

    /// Supprime les fichiers intermédiaires laissés par un processus yt-dlp interrompu ou en échec.
    async fn remove_intermediate_files(&self) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if is_ytdlp_intermediate_file(name, &self.base_name) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

/// Supprime les fichiers partiels d'un téléchargement abandonné (yt-dlp ou streaming direct),
/// par exemple quand un téléchargement en pause est annulé.
pub async fn discard_partial_download(target_path: &Path) {
    if let Some(output) = YtdlpOutput::for_target(target_path) {
        output.remove_intermediate_files().await;
    }
}

/// Reconnaît les fichiers intermédiaires de yt-dlp pour un nom de base donné.
///
/// yt-dlp écrit `<nom>.<ext>.part`, `<nom>.<ext>.ytdl`, des fragments `.part-FragN` et, pour les
/// formats séparés en attente de fusion, `<nom>.f<id>.<ext>`. Le fichier final `<nom>.<ext>`
/// n'est jamais concerné, afin de ne pas supprimer un téléchargement terminé.
fn is_ytdlp_intermediate_file(file_name: &str, base_name: &str) -> bool {
    let Some(rest) = file_name
        .strip_prefix(base_name)
        .and_then(|r| r.strip_prefix('.'))
    else {
        return false;
    };

    if rest.ends_with(".part") || rest.ends_with(".ytdl") || rest.contains(".part-Frag") {
        return true;
    }

    // Flux séparé : `f<numéro>[-variante].<ext>` (ex. `f251.webm`, `f251-drc.webm`). Le numéro
    // est exigé : le nom de base vient d'un titre, et « Titre.final.mp4 » doit rester intact.
    let Some((format_part, ext)) = rest.split_once('.') else {
        return false;
    };
    let Some(format_id) = format_part.strip_prefix('f') else {
        return false;
    };
    let (number, variant) = format_id.split_once('-').unwrap_or((format_id, ""));
    !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && variant.chars().all(|c| c.is_ascii_alphanumeric())
        && !ext.is_empty()
        && !ext.contains('.')
}

/// Lit stderr jusqu'à sa fermeture en ne gardant que les dernières lignes.
async fn collect_stderr_tail(stderr: tokio::process::ChildStderr) -> String {
    let mut lines = BufReader::new(stderr).lines();
    let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL_LINES);
    while let Ok(Some(line)) = lines.next_line().await {
        if tail.len() == STDERR_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    Vec::from(tail).join("\n")
}

/// Traduit la sortie d'erreur de yt-dlp en erreur compréhensible.
///
/// Les avertissements (`WARNING:`) sont écartés quand des lignes `ERROR:` existent, pour que
/// leur contenu ne fausse pas le diagnostic.
fn ytdlp_failure_error(stderr_tail: &str, requires_single_file: bool) -> CommandError {
    let error_lines: Vec<&str> = stderr_tail
        .lines()
        .filter(|l| l.starts_with("ERROR:"))
        .collect();
    let relevant = if error_lines.is_empty() {
        stderr_tail.to_string()
    } else {
        error_lines.join("\n")
    };

    if requires_single_file
        && relevant
            .to_lowercase()
            .contains("requested format is not available")
    {
        return CommandError::DownloadFailed(
            "Cette vidéo n'existe qu'en flux audio et vidéo séparés. Installez ffmpeg pour permettre leur fusion, puis réessayez.".to_string(),
        );
    }

    if relevant.trim().is_empty() {
        return CommandError::DownloadFailed(
            "Le téléchargement via yt-dlp s'est interrompu anormalement.".to_string(),
        );
    }

    CommandError::from_ytdlp_stderr(&relevant)
}

/// Localise `ffmpeg`, nécessaire à yt-dlp pour fusionner audio et vidéo ou convertir en MP3.
fn find_ffmpeg_binary() -> Option<PathBuf> {
    find_executable(
        "ffmpeg",
        &[
            PathBuf::from("/usr/local/bin/ffmpeg"),
            PathBuf::from("/usr/bin/ffmpeg"),
            PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        ],
    )
}

/// Localise un runtime JavaScript pour le solveur de défis YouTube de yt-dlp (EJS).
///
/// Sans runtime, YouTube ne livre qu'une partie des formats (souvent aucun format audio+vidéo
/// combiné). yt-dlp n'active que deno par défaut : node et bun doivent lui être indiqués.
fn find_js_runtime() -> Option<(&'static str, PathBuf)> {
    let home_install = |relative: &str| -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| vec![home.join(relative)])
            .unwrap_or_default()
    };

    find_executable("deno", &home_install(".deno/bin/deno"))
        .map(|path| ("deno", path))
        .or_else(|| {
            let mut locations = vec![
                PathBuf::from("/usr/local/bin/node"),
                PathBuf::from("/usr/bin/node"),
                PathBuf::from("/opt/homebrew/bin/node"),
            ];
            locations.extend(user_node_installs());
            find_executable("node", &locations).map(|path| ("node", path))
        })
        .or_else(|| find_executable("bun", &home_install(".bun/bin/bun")).map(|path| ("bun", path)))
}

/// Installations de node propres à l'utilisateur (volta, nvm, fnm).
///
/// Ces gestionnaires n'ajoutent node au `PATH` que dans un terminal : une application lancée
/// depuis le menu du bureau ne le verrait pas, et yt-dlp tournerait sans moteur JavaScript.
fn user_node_installs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let mut candidates = vec![home.join(".volta/bin/node")];
    let versioned_installs = [
        (home.join(".nvm/versions/node"), "bin/node"),
        (
            home.join(".local/share/fnm/node-versions"),
            "installation/bin/node",
        ),
    ];
    for (versions_dir, binary) in versioned_installs {
        let Ok(entries) = std::fs::read_dir(&versions_dir) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        // La version la plus récente d'abord
        versions.sort_by_key(|dir| std::cmp::Reverse(node_version_key(dir)));
        candidates.extend(versions.into_iter().map(|dir| dir.join(binary)));
    }
    candidates
}

/// Clé de tri d'un dossier de version nommé `v24.20.0` : `[24, 20, 0]`.
fn node_version_key(dir: &Path) -> Vec<u64> {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            name.trim_start_matches('v')
                .split('.')
                .map(|part| part.parse().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}

/// Recherche un exécutable dans le `PATH`, puis dans les emplacements usuels fournis.
fn find_executable(name: &str, usual_locations: &[PathBuf]) -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(&file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    usual_locations.iter().find(|p| p.is_file()).cloned()
}

/// Prépare une commande yt-dlp avec le runtime JavaScript détecté, commune à l'analyse et au
/// téléchargement pour que les deux voient les mêmes formats.
fn ytdlp_command(ytdlp_bin: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(ytdlp_bin);
    if let Some((runtime, path)) = find_js_runtime() {
        cmd.arg("--js-runtimes")
            .arg(format!("{}:{}", runtime, path.display()));
    }
    cmd
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

    #[test]
    fn test_is_ytdlp_intermediate_file() {
        let base = "video_ab12cd34";
        assert!(is_ytdlp_intermediate_file("video_ab12cd34.mp4.part", base));
        assert!(is_ytdlp_intermediate_file("video_ab12cd34.webm.ytdl", base));
        assert!(is_ytdlp_intermediate_file(
            "video_ab12cd34.f137.mp4.part",
            base
        ));
        assert!(is_ytdlp_intermediate_file(
            "video_ab12cd34.mp4.part-Frag12",
            base
        ));
        // Flux séparés laissés sans fusion quand ffmpeg est absent
        assert!(is_ytdlp_intermediate_file("video_ab12cd34.f251.webm", base));
        assert!(is_ytdlp_intermediate_file("video_ab12cd34.f395.mp4", base));
        assert!(is_ytdlp_intermediate_file(
            "video_ab12cd34.f251-drc.webm",
            base
        ));

        // Fichiers finalisés ou appartenant à un autre téléchargement : jamais supprimés
        assert!(!is_ytdlp_intermediate_file("video_ab12cd34.mp4", base));
        assert!(!is_ytdlp_intermediate_file("video_ab12cd34.webm", base));
        assert!(!is_ytdlp_intermediate_file(
            "video_ab12cd34.final.mp4",
            base
        ));
        assert!(!is_ytdlp_intermediate_file("video_ffffffff.mp4.part", base));
        assert!(!is_ytdlp_intermediate_file(
            "video_ab12cd345.mp4.part",
            base
        ));
    }

    #[test]
    fn test_ytdlp_output_template_lets_ytdlp_choose_extension() {
        let output = YtdlpOutput::for_target(Path::new("/tmp/dl/video_ab12cd34.mp4")).unwrap();
        assert_eq!(output.base_name, "video_ab12cd34");
        assert_eq!(output.template(), "/tmp/dl/video_ab12cd34.%(ext)s");
    }

    #[test]
    fn test_ytdlp_output_template_escapes_percent_and_keeps_dotted_names() {
        let output = YtdlpOutput::for_target(Path::new("/tmp/100% films/Mr. Bean")).unwrap();
        assert_eq!(output.base_name, "Mr. Bean");
        assert_eq!(output.template(), "/tmp/100%% films/Mr. Bean.%(ext)s");
    }

    #[test]
    fn test_ytdlp_failure_error_explains_missing_ffmpeg() {
        let stderr = "WARNING: [youtube] No supported JavaScript runtime could be found.\n\
                      ERROR: [youtube] jNQXAC9IVRw: Requested format is not available. Use --list-formats for a list of available formats";
        match ytdlp_failure_error(stderr, true) {
            CommandError::DownloadFailed(msg) => assert!(msg.contains("ffmpeg")),
            other => panic!("Erreur inattendue : {:?}", other),
        }
    }

    #[test]
    fn test_ytdlp_failure_error_ignores_warnings_when_errors_exist() {
        // L'avertissement contient « not found » (vidéo introuvable) mais la vraie cause est une
        // vidéo privée.
        let stderr =
            "WARNING: [youtube] subtitles not found\nERROR: [youtube] abc: Private video. Sign in";
        assert!(matches!(
            ytdlp_failure_error(stderr, false),
            CommandError::DownloadFailed(_)
        ));
    }

    #[test]
    fn test_node_version_key_orders_newest_first() {
        let mut dirs = [
            PathBuf::from("/n/v18.19.0"),
            PathBuf::from("/n/v24.20.0"),
            PathBuf::from("/n/v20.11.1"),
        ];
        dirs.sort_by_key(|dir| std::cmp::Reverse(node_version_key(dir)));
        assert_eq!(dirs[0], PathBuf::from("/n/v24.20.0"));
        assert_eq!(dirs[2], PathBuf::from("/n/v18.19.0"));
    }

    #[test]
    fn test_find_executable_uses_usual_locations() {
        let dir = std::env::temp_dir().join("ktk-find-executable-test");
        std::fs::create_dir_all(&dir).unwrap();
        let fake_bin = dir.join("ktk-fake-runtime");
        std::fs::write(&fake_bin, "").unwrap();

        assert_eq!(
            find_executable("ktk-fake-runtime", std::slice::from_ref(&fake_bin)),
            Some(fake_bin.clone())
        );
        assert_eq!(
            find_executable("ktk-absent-runtime", &[dir.join("ktk-absent-runtime")]),
            None
        );

        let _ = std::fs::remove_file(&fake_bin);
    }

    #[test]
    fn test_is_direct_media() {
        assert!(is_direct_media("video/mp4", "/media/clip"));
        assert!(is_direct_media("audio/mpeg; charset=binary", "/a"));
        assert!(is_direct_media(
            "application/octet-stream",
            "/files/clip.MP4"
        ));

        assert!(!is_direct_media(
            "text/html; charset=utf-8",
            "/watch/clip.mp4"
        ));
        assert!(!is_direct_media(
            "application/octet-stream",
            "/files/setup.exe"
        ));
        assert!(!is_direct_media("", "/files/clip.mp4"));
    }

    #[test]
    fn test_media_title_from_path() {
        assert_eq!(
            media_title_from_path("/media/big_buck_bunny.mp4"),
            "big_buck_bunny"
        );
        assert_eq!(media_title_from_path("/"), "Vidéo Web");
    }

    #[test]
    fn test_ytdlp_failure_error_without_output() {
        assert!(matches!(
            ytdlp_failure_error("", false),
            CommandError::DownloadFailed(_)
        ));
    }

    fn create_mock_script(content: &str) -> PathBuf {
        let unique = format!("ktk-mock-ytdlp-{}.sh", uuid::Uuid::new_v4());
        let path = std::env::temp_dir().join(unique);
        std::fs::write(&path, content).expect("Écriture du script mock yt-dlp");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn test_mock_ytdlp_extract_success() {
        let script = r#"#!/bin/sh
cat << 'EOF'
{
  "id": "mock_abc123",
  "title": "Vidéo documentaire HD",
  "description": "Description de test complète",
  "uploader": "Studio Test",
  "thumbnail": "https://example.com/thumb.jpg",
  "duration": 185,
  "extractor": "youtube",
  "formats": [
    {
      "format_id": "137",
      "ext": "mp4",
      "height": 1080,
      "width": 1920,
      "vcodec": "avc1.640028",
      "acodec": "none",
      "filesize": 52428800
    },
    {
      "format_id": "140",
      "ext": "m4a",
      "vcodec": "none",
      "acodec": "mp4a.40.2",
      "filesize": 5242880
    }
  ]
}
EOF
exit 0
"#;
        let mock_path = create_mock_script(script);

        tauri::async_runtime::block_on(async {
            let res =
                extract_with_ytdlp(&mock_path, "https://www.youtube.com/watch?v=mock_abc123").await;
            let _ = std::fs::remove_file(&mock_path);

            assert!(res.is_ok(), "L'extraction mock doit réussir : {:?}", res);
            let meta = res.unwrap();
            assert_eq!(meta.id, "mock_abc123");
            assert_eq!(meta.title, "Vidéo documentaire HD");
            assert_eq!(meta.author.as_deref(), Some("Studio Test"));
            assert_eq!(meta.duration_seconds, Some(185));
            assert_eq!(meta.source_platform, "YouTube");
            assert!(!meta.available_formats.is_empty());
        });
    }

    #[test]
    fn test_mock_ytdlp_extract_private_video() {
        let script = r#"#!/bin/sh
echo "ERROR: [youtube] private_id: Private video. Sign in to confirm you have access" >&2
exit 1
"#;
        let mock_path = create_mock_script(script);

        tauri::async_runtime::block_on(async {
            let res =
                extract_with_ytdlp(&mock_path, "https://www.youtube.com/watch?v=private_id").await;
            let _ = std::fs::remove_file(&mock_path);

            assert!(res.is_err());
            match res.unwrap_err() {
                CommandError::DownloadFailed(msg) => {
                    assert!(
                        msg.to_lowercase().contains("privée")
                            || msg.to_lowercase().contains("private")
                    );
                }
                other => panic!("Erreur attendue DownloadFailed, reçu : {:?}", other),
            }
        });
    }

    #[test]
    fn test_mock_ytdlp_extract_malformed_json() {
        let script = r#"#!/bin/sh
echo "Garbage non-json output !!! {{"
exit 0
"#;
        let mock_path = create_mock_script(script);

        tauri::async_runtime::block_on(async {
            let res = extract_with_ytdlp(&mock_path, "https://example.com/video").await;
            let _ = std::fs::remove_file(&mock_path);

            assert!(res.is_err());
            assert!(matches!(
                res.unwrap_err(),
                CommandError::ExtractionFailed(_)
            ));
        });
    }

    #[test]
    fn test_mock_ytdlp_download_success() {
        let script = r#"#!/bin/sh
echo "ktk: 20.0%| 2.00MiB/s|00:20|1048576"
echo "ktk: 60.0%| 2.50MiB/s|00:10|1048576"
echo "ktk:100.0%| 3.00MiB/s|00:00|1048576"

TARGET_FILE=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    TARGET_FILE="$2"
    shift 2
  else
    shift 1
  fi
done

ACTUAL_FILE=$(echo "$TARGET_FILE" | sed 's/%(ext)s/mp4/')
echo "Simulated binary video payload" > "$ACTUAL_FILE"
echo "ktk-file:$ACTUAL_FILE"
exit 0
"#;
        let mock_path = create_mock_script(script);
        let app = tauri::test::mock_app();
        let target_dir = std::env::temp_dir().join(format!("ktk-dl-test-{}", uuid::Uuid::new_v4()));
        let target_path = target_dir.join("test_video.mp4");

        tauri::async_runtime::block_on(async {
            let res = download_with_ytdlp_binary(
                app.handle(),
                "dl-test-success",
                "https://example.com/video.mp4",
                QualityPreset::Best,
                &target_path,
                None,
                &mock_path,
            )
            .await;

            let _ = std::fs::remove_file(&mock_path);

            assert!(
                res.is_ok(),
                "Le téléchargement mocké doit réussir : {:?}",
                res
            );
            let (final_path, file_size) = res.unwrap();
            assert!(final_path.exists());
            assert!(file_size > 0);

            let _ = std::fs::remove_file(&final_path);
            let _ = std::fs::remove_dir(&target_dir);
        });
    }

    #[test]
    fn test_mock_ytdlp_download_cancellation() {
        let script = r#"#!/bin/sh
TARGET_FILE=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    TARGET_FILE="$2"
    shift 2
  else
    shift 1
  fi
done

BASE=$(echo "$TARGET_FILE" | sed 's/\.%(ext)s//')
PART_FILE="${BASE}.mp4.part"
touch "$PART_FILE"
echo "ktk: 15.0%| 1.00MiB/s|01:00|10485760"

while true; do
  sleep 0.05
done
"#;
        let mock_path = create_mock_script(script);
        let app = tauri::test::mock_app();
        let target_dir =
            std::env::temp_dir().join(format!("ktk-dl-cancel-{}", uuid::Uuid::new_v4()));
        let target_path = target_dir.join("test_cancel.mp4");
        let control = Arc::new(DownloadControl::default());
        let control_clone = Arc::clone(&control);

        tauri::async_runtime::block_on(async {
            let handle = app.handle().clone();
            let target_clone = target_path.clone();
            let mock_clone = mock_path.clone();

            let task = tokio::spawn(async move {
                download_with_ytdlp_binary(
                    &handle,
                    "dl-test-cancel",
                    "https://example.com/video.mp4",
                    QualityPreset::Best,
                    &target_clone,
                    Some(control_clone),
                    &mock_clone,
                )
                .await
            });

            // Laisse le script démarrer et créer le fichier partiel
            tokio::time::sleep(Duration::from_millis(150)).await;
            control.request_cancel();

            let res = task.await.expect("Join task");
            let _ = std::fs::remove_file(&mock_path);

            assert!(matches!(res, Err(CommandError::DownloadCancelled)));

            // Vérifie que les fichiers temporaires .part ont été nettoyés
            let intermediate = target_dir.join("test_cancel.mp4.part");
            assert!(
                !intermediate.exists(),
                "Le fichier .part doit être nettoyé après annulation"
            );

            let _ = std::fs::remove_dir(&target_dir);
        });
    }

    #[test]
    fn test_mock_ytdlp_download_pause() {
        let script = r#"#!/bin/sh
TARGET_FILE=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    TARGET_FILE="$2"
    shift 2
  else
    shift 1
  fi
done

BASE=$(echo "$TARGET_FILE" | sed 's/\.%(ext)s//')
PART_FILE="${BASE}.mp4.part"
touch "$PART_FILE"
echo "ktk: 30.0%| 1.00MiB/s|00:45|10485760"

while true; do
  sleep 0.05
done
"#;
        let mock_path = create_mock_script(script);
        let app = tauri::test::mock_app();
        let target_dir =
            std::env::temp_dir().join(format!("ktk-dl-pause-{}", uuid::Uuid::new_v4()));
        let target_path = target_dir.join("test_pause.mp4");
        let control = Arc::new(DownloadControl::default());
        let control_clone = Arc::clone(&control);

        tauri::async_runtime::block_on(async {
            let handle = app.handle().clone();
            let target_clone = target_path.clone();
            let mock_clone = mock_path.clone();

            let task = tokio::spawn(async move {
                download_with_ytdlp_binary(
                    &handle,
                    "dl-test-pause",
                    "https://example.com/video.mp4",
                    QualityPreset::Best,
                    &target_clone,
                    Some(control_clone),
                    &mock_clone,
                )
                .await
            });

            tokio::time::sleep(Duration::from_millis(150)).await;
            control.request_pause();

            let res = task.await.expect("Join task");
            let _ = std::fs::remove_file(&mock_path);

            assert!(matches!(res, Err(CommandError::DownloadPaused)));

            // Lors d'une pause, le fichier partiel .part DOIT être conservé pour la reprise !
            let intermediate = target_dir.join("test_pause.mp4.part");
            assert!(
                intermediate.exists(),
                "Le fichier .part doit être conservé lors d'une pause"
            );

            let _ = std::fs::remove_file(&intermediate);
            let _ = std::fs::remove_dir(&target_dir);
        });
    }

    #[test]
    fn test_mock_ytdlp_download_failure_cleanup() {
        let script = r#"#!/bin/sh
TARGET_FILE=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    TARGET_FILE="$2"
    shift 2
  else
    shift 1
  fi
done

BASE=$(echo "$TARGET_FILE" | sed 's/\.%(ext)s//')
PART_FILE="${BASE}.mp4.part"
touch "$PART_FILE"
echo "ERROR: [generic] HTTP Error 403: Forbidden" >&2
exit 1
"#;
        let mock_path = create_mock_script(script);
        let app = tauri::test::mock_app();
        let target_dir = std::env::temp_dir().join(format!("ktk-dl-fail-{}", uuid::Uuid::new_v4()));
        let target_path = target_dir.join("test_fail.mp4");

        tauri::async_runtime::block_on(async {
            let res = download_with_ytdlp_binary(
                app.handle(),
                "dl-test-fail",
                "https://example.com/forbidden.mp4",
                QualityPreset::Best,
                &target_path,
                None,
                &mock_path,
            )
            .await;

            let _ = std::fs::remove_file(&mock_path);

            assert!(res.is_err());
            assert!(matches!(
                res.unwrap_err(),
                CommandError::ExtractionFailed(_) | CommandError::DownloadFailed(_)
            ));

            // Vérifie que le fichier partiel a été nettoyé
            let intermediate = target_dir.join("test_fail.mp4.part");
            assert!(
                !intermediate.exists(),
                "Le fichier .part doit être nettoyé en cas d'échec"
            );

            let _ = std::fs::remove_dir(&target_dir);
        });
    }
}
