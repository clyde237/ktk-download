use serde_json::Value;

use crate::error::CommandError;
use crate::models::VideoFormat;

/// Qualités proposées à l'utilisateur.
///
/// Le frontend n'envoie que l'identifiant d'un préréglage : le sélecteur de formats yt-dlp est
/// construit ici, jamais repris tel quel depuis l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    Best,
    P1080,
    P720,
    P480,
    AudioMp3,
}

/// Préréglages vidéo plafonnés, du plus haut au plus bas.
const CAPPED_VIDEO_PRESETS: [QualityPreset; 3] = [
    QualityPreset::P1080,
    QualityPreset::P720,
    QualityPreset::P480,
];

impl QualityPreset {
    pub fn parse(id: &str) -> Result<Self, CommandError> {
        match id.trim() {
            "" | "best" => Ok(Self::Best),
            "1080p" => Ok(Self::P1080),
            "720p" => Ok(Self::P720),
            "480p" => Ok(Self::P480),
            "audio_only" => Ok(Self::AudioMp3),
            other => Err(CommandError::DownloadFailed(format!(
                "Qualité inconnue : « {} ».",
                other
            ))),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::P1080 => "1080p",
            Self::P720 => "720p",
            Self::P480 => "480p",
            Self::AudioMp3 => "audio_only",
        }
    }

    pub fn max_height(self) -> Option<u64> {
        match self {
            Self::P1080 => Some(1080),
            Self::P720 => Some(720),
            Self::P480 => Some(480),
            Self::Best | Self::AudioMp3 => None,
        }
    }

    pub fn is_audio_only(self) -> bool {
        self == Self::AudioMp3
    }

    /// Arguments de sélection de format pour yt-dlp.
    ///
    /// Sans ffmpeg (`can_merge` faux), yt-dlp ne peut pas fusionner audio et vidéo : seuls les
    /// formats contenant déjà les deux flux sont demandés.
    pub fn ytdlp_args(self, can_merge: bool) -> Vec<String> {
        let selector = match (self, self.max_height(), can_merge) {
            (Self::AudioMp3, _, _) => {
                return ["-f", "ba/b", "-x", "--audio-format", "mp3"]
                    .map(String::from)
                    .to_vec();
            }
            (_, Some(h), true) => format!("bv*[height<={h}]+ba/b[height<={h}]"),
            (_, Some(h), false) => format!("b[height<={h}]"),
            // Sélection par défaut de yt-dlp : meilleure vidéo + meilleur audio
            (_, None, true) => return Vec::new(),
            (_, None, false) => "b".to_string(),
        };
        vec!["-f".to_string(), selector]
    }
}

/// Construit la liste des qualités réellement disponibles à partir des formats renvoyés par
/// `yt-dlp --dump-single-json`, avec une taille estimée quand yt-dlp la connaît.
pub fn quality_options(raw_formats: &[Value]) -> Vec<VideoFormat> {
    let video_heights = raw_formats
        .iter()
        .filter(|f| has_video(f))
        .filter_map(|f| f["height"].as_u64());
    let max_height = video_heights.max();
    // Codecs absents (fichier direct analysé par l'extracteur générique) : on ne sait rien,
    // donc on ne retire aucune option. Seul « none » explicite prouve l'absence d'un flux.
    let audio_only_source = !raw_formats.is_empty()
        && raw_formats
            .iter()
            .all(|f| f["vcodec"].as_str() == Some("none"));
    let may_have_audio = raw_formats.is_empty()
        || raw_formats
            .iter()
            .any(|f| f["acodec"].as_str() != Some("none"));

    let mut options = Vec::new();

    if !audio_only_source {
        options.push(VideoFormat {
            format_id: QualityPreset::Best.id().to_string(),
            label: "Meilleure qualité".to_string(),
            resolution: max_height.map(|h| format!("{}p", h)),
            extension: "mp4".to_string(),
            file_size_bytes: estimate_video_size(raw_formats, None),
            has_video: true,
            has_audio: true,
            direct_url: None,
        });
    }

    if let Some(max) = max_height {
        for preset in CAPPED_VIDEO_PRESETS {
            let Some(cap) = preset.max_height() else {
                continue;
            };
            // Un plafond n'est proposé que si la vidéo l'atteint : « 1080p » doit valoir 1080p.
            if max >= cap {
                options.push(VideoFormat {
                    format_id: preset.id().to_string(),
                    label: preset.id().to_string(),
                    resolution: Some(preset.id().to_string()),
                    extension: "mp4".to_string(),
                    file_size_bytes: estimate_video_size(raw_formats, Some(cap)),
                    has_video: true,
                    has_audio: true,
                    direct_url: None,
                });
            }
        }
    }

    if may_have_audio {
        options.push(VideoFormat {
            format_id: QualityPreset::AudioMp3.id().to_string(),
            label: "Audio MP3".to_string(),
            resolution: None,
            extension: "mp3".to_string(),
            file_size_bytes: None,
            has_video: false,
            has_audio: true,
            direct_url: None,
        });
    }

    options
}

fn has_video(format: &Value) -> bool {
    format["vcodec"].as_str().is_some_and(|v| v != "none")
}

fn has_audio(format: &Value) -> bool {
    format["acodec"].as_str().is_some_and(|a| a != "none")
}

fn size_of(format: &Value) -> Option<u64> {
    format["filesize"]
        .as_u64()
        .or_else(|| format["filesize_approx"].as_u64())
}

fn bitrate_of(format: &Value) -> f64 {
    format["tbr"].as_f64().unwrap_or(0.0)
}

/// Rang de préférence d'un codec vidéo, dans l'ordre du tri par défaut de yt-dlp.
fn video_codec_rank(format: &Value) -> u8 {
    let codec = format["vcodec"].as_str().unwrap_or_default();
    if codec.starts_with("av01") {
        4
    } else if codec.starts_with("vp9") || codec.starts_with("vp09") {
        3
    } else if codec.starts_with("hev") || codec.starts_with("hvc") || codec.starts_with("h265") {
        2
    } else {
        1
    }
}

/// Rang de préférence d'un flux audio : piste normale avant variante « drc », puis codec.
fn audio_rank(format: &Value) -> (bool, u8) {
    let is_drc = format["format_id"]
        .as_str()
        .is_some_and(|id| id.ends_with("-drc"));
    let codec = format["acodec"].as_str().unwrap_or_default();
    let codec_rank = if codec.starts_with("opus") {
        3
    } else if codec.starts_with("mp4a") || codec.starts_with("aac") {
        2
    } else {
        1
    };
    (!is_drc, codec_rank)
}

fn compare_f64(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Estime la taille du fichier que yt-dlp produira pour un plafond de hauteur donné, en imitant
/// son choix par défaut : définition, puis taille connue (flux HTTP plutôt que HLS), puis codec,
/// puis débit. Meilleur flux vidéo seul + meilleur flux audio seul, ou à défaut format combiné.
fn estimate_video_size(raw_formats: &[Value], max_height: Option<u64>) -> Option<u64> {
    let fits =
        |f: &&Value| max_height.is_none_or(|cap| f["height"].as_u64().is_some_and(|h| h <= cap));
    let video_preference = |a: &&Value, b: &&Value| {
        let key = |f: &Value| {
            (
                f["height"].as_u64().unwrap_or(0),
                size_of(f).is_some(),
                video_codec_rank(f),
            )
        };
        key(a)
            .cmp(&key(b))
            .then_with(|| compare_f64(bitrate_of(a), bitrate_of(b)))
    };

    let video_only = raw_formats
        .iter()
        .filter(|f| has_video(f) && !has_audio(f))
        .filter(fits)
        .max_by(video_preference);
    let audio_only = raw_formats
        .iter()
        .filter(|f| has_audio(f) && !has_video(f))
        .max_by(|a, b| {
            audio_rank(a)
                .cmp(&audio_rank(b))
                .then_with(|| compare_f64(bitrate_of(a), bitrate_of(b)))
        });

    if let (Some(video), Some(audio)) = (video_only, audio_only) {
        return Some(size_of(video)? + size_of(audio)?);
    }

    raw_formats
        .iter()
        .filter(|f| has_video(f) && has_audio(f))
        .filter(fits)
        .max_by(video_preference)
        .and_then(size_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_formats() -> Vec<Value> {
        vec![
            json!({"format_id": "251", "vcodec": "none", "acodec": "opus", "tbr": 130.0, "filesize": 3_000_000}),
            json!({"format_id": "18", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "tbr": 500.0, "filesize": 9_000_000}),
            json!({"format_id": "135", "vcodec": "avc1", "acodec": "none", "height": 480, "tbr": 900.0, "filesize": 10_000_000}),
            json!({"format_id": "136", "vcodec": "avc1", "acodec": "none", "height": 720, "tbr": 1800.0, "filesize": 20_000_000}),
        ]
    }

    #[test]
    fn test_parse_accepts_only_known_presets() {
        assert_eq!(QualityPreset::parse("best").unwrap(), QualityPreset::Best);
        assert_eq!(QualityPreset::parse("").unwrap(), QualityPreset::Best);
        assert_eq!(QualityPreset::parse("720p").unwrap(), QualityPreset::P720);
        assert_eq!(
            QualityPreset::parse("audio_only").unwrap(),
            QualityPreset::AudioMp3
        );
        // Un sélecteur yt-dlp brut ou une option ne doit jamais passer
        assert!(QualityPreset::parse("bv*+ba").is_err());
        assert!(QualityPreset::parse("--exec rm").is_err());
    }

    #[test]
    fn test_ytdlp_args_with_and_without_ffmpeg() {
        assert!(QualityPreset::Best.ytdlp_args(true).is_empty());
        assert_eq!(QualityPreset::Best.ytdlp_args(false), vec!["-f", "b"]);
        assert_eq!(
            QualityPreset::P720.ytdlp_args(true),
            vec!["-f", "bv*[height<=720]+ba/b[height<=720]"]
        );
        assert_eq!(
            QualityPreset::P720.ytdlp_args(false),
            vec!["-f", "b[height<=720]"]
        );
        assert_eq!(
            QualityPreset::AudioMp3.ytdlp_args(true),
            vec!["-f", "ba/b", "-x", "--audio-format", "mp3"]
        );
    }

    #[test]
    fn test_quality_options_only_offer_reachable_heights() {
        let options = quality_options(&sample_formats());
        let ids: Vec<&str> = options.iter().map(|o| o.format_id.as_str()).collect();
        // Vidéo plafonnée à 720p : pas de « 1080p » trompeur
        assert_eq!(ids, vec!["best", "720p", "480p", "audio_only"]);
        assert_eq!(options[0].resolution.as_deref(), Some("720p"));
    }

    #[test]
    fn test_quality_options_estimate_sizes() {
        let options = quality_options(&sample_formats());
        let size = |id: &str| {
            options
                .iter()
                .find(|o| o.format_id == id)
                .and_then(|o| o.file_size_bytes)
        };
        assert_eq!(size("best"), Some(23_000_000));
        assert_eq!(size("480p"), Some(13_000_000));
    }

    #[test]
    fn test_size_estimate_follows_ytdlp_preferences() {
        let formats = vec![
            // HLS au plus haut débit mais sans taille : yt-dlp ne le choisit pas
            json!({"format_id": "628", "vcodec": "vp09", "acodec": "none", "height": 2160, "tbr": 27987.0}),
            json!({"format_id": "315", "vcodec": "vp9", "acodec": "none", "height": 2160, "tbr": 17174.0, "filesize": 1_362_000_000}),
            json!({"format_id": "401", "vcodec": "av01.0.12M.08", "acodec": "none", "height": 2160, "tbr": 8981.0, "filesize": 712_000_000}),
            json!({"format_id": "251-drc", "vcodec": "none", "acodec": "opus", "tbr": 129.3, "filesize": 10_300_000}),
            json!({"format_id": "251", "vcodec": "none", "acodec": "opus", "tbr": 128.6, "filesize": 10_200_000}),
            json!({"format_id": "258", "vcodec": "none", "acodec": "mp4a.40.2", "tbr": 387.8, "filesize": 30_700_000}),
        ];
        assert_eq!(estimate_video_size(&formats, None), Some(722_200_000));
    }

    #[test]
    fn test_quality_options_unknown_codecs_keep_defaults() {
        // Fichier direct : l'extracteur générique ne renseigne ni codec ni hauteur
        let options = quality_options(&[json!({"format_id": "mp4", "ext": "mp4"})]);
        let ids: Vec<&str> = options.iter().map(|o| o.format_id.as_str()).collect();
        assert_eq!(ids, vec!["best", "audio_only"]);
    }

    #[test]
    fn test_quality_options_audio_only_source() {
        let options = quality_options(&[json!({"vcodec": "none", "acodec": "mp3", "tbr": 128.0})]);
        let ids: Vec<&str> = options.iter().map(|o| o.format_id.as_str()).collect();
        assert_eq!(ids, vec!["audio_only"]);
    }
}
