use std::net::{Ipv4Addr, Ipv6Addr};
use url::Url;

use crate::error::CommandError;

/// Liste des paramètres de tracking usuels à retirer lors de l'assainissement de l'URL.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "igshid",
    "si",
    "feature",
    "ref",
    "ref_src",
];

/// Plateformes vidéo reconnues et catégorisées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoPlatform {
    YouTube,
    Vimeo,
    Dailymotion,
    TikTok,
    TwitterX,
    Reddit,
    GenericWeb(String),
}

impl VideoPlatform {
    pub fn as_str(&self) -> &str {
        match self {
            VideoPlatform::YouTube => "YouTube",
            VideoPlatform::Vimeo => "Vimeo",
            VideoPlatform::Dailymotion => "Dailymotion",
            VideoPlatform::TikTok => "TikTok",
            VideoPlatform::TwitterX => "Twitter / X",
            VideoPlatform::Reddit => "Reddit",
            VideoPlatform::GenericWeb(_) => "Flux Web / Direct",
        }
    }
}

/// Résultat de la validation et de l'assainissement d'une URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedUrl {
    /// URL nettoyée et canonique
    pub canonical_url: String,
    /// Nom d'hôte (domaine)
    pub domain: String,
    /// Plateforme identifiée
    pub platform: VideoPlatform,
}

/// Valide et assainit une URL soumise par l'utilisateur.
///
/// Règles appliquées :
/// - Rejet des chaînes vides ou excessivement longues (> 2048 caractères).
/// - Nettoyage des caractères invisibles et espaces de début/fin.
/// - Restriction stricte aux protocoles sécurisés `http` et `https` (rejet de `file:`, `javascript:`, etc.).
/// - Protection SSRF : interdiction des adresses localhost, loopback, et IPs privées/locales.
/// - Retrait des paramètres de tracking publicitaires (`utm_*`, `si`, `fbclid`, etc.).
/// - Normalisation canonique des URLs YouTube (`youtu.be`, `/shorts/`, `/embed/`).
pub fn validate_and_sanitize_url(raw_url: &str) -> Result<ValidatedUrl, CommandError> {
    let trimmed = raw_url.trim();

    if trimmed.is_empty() {
        return Err(CommandError::InvalidUrl(
            "L'adresse URL ne peut pas être vide.".to_string(),
        ));
    }

    if trimmed.len() > 2048 {
        return Err(CommandError::InvalidUrl(
            "L'adresse URL dépasse la longueur maximale autorisée (2048 caractères).".to_string(),
        ));
    }

    // Détection des caractères de contrôle ou bytes nuls
    if trimmed.contains('\0') || trimmed.chars().any(|c| c.is_control()) {
        return Err(CommandError::InvalidUrl(
            "L'adresse URL contient des caractères de contrôle invalides.".to_string(),
        ));
    }

    let parsed = Url::parse(trimmed)
        .map_err(|e| CommandError::InvalidUrl(format!("Format d'adresse invalide : {}", e)))?;

    // Validation stricte du protocole
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(CommandError::InvalidUrl(format!(
            "Le protocole '{}' n'est pas autorisé. Seuls HTTP et HTTPS sont pris en charge.",
            scheme
        )));
    }

    // Validation du nom d'hôte / domaine
    let host_str = match parsed.host() {
        Some(url::Host::Domain(d)) => d.to_lowercase(),
        Some(url::Host::Ipv4(ip)) => {
            if is_private_or_local_ipv4(ip) {
                return Err(CommandError::InvalidUrl(
                    "Les adresses réseau locales et privées ne sont pas autorisées (protection SSRF).".to_string(),
                ));
            }
            ip.to_string()
        }
        Some(url::Host::Ipv6(ip)) => {
            if is_private_or_local_ipv6(ip) {
                return Err(CommandError::InvalidUrl(
                    "Les adresses réseau locales et privées ne sont pas autorisées (protection SSRF).".to_string(),
                ));
            }
            ip.to_string()
        }
        None => {
            return Err(CommandError::InvalidUrl(
                "L'URL doit comporter un nom d'hôte valide.".to_string(),
            ));
        }
    };

    if host_str == "localhost" || host_str.ends_with(".localhost") || host_str.ends_with(".local") {
        return Err(CommandError::InvalidUrl(
            "L'accès aux domaines locaux 'localhost' n'est pas autorisé.".to_string(),
        ));
    }

    // Détection de la plateforme
    let platform = detect_platform(&host_str);

    // Normalisation et retrait des paramètres de tracking
    let canonical_url = canonicalize_url(&parsed, &platform, &host_str);

    Ok(ValidatedUrl {
        canonical_url,
        domain: host_str,
        platform,
    })
}

/// Identifie la plateforme vidéo à partir du nom d'hôte.
fn detect_platform(host: &str) -> VideoPlatform {
    if host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtu.be"
        || host == "youtube-nocookie.com"
    {
        VideoPlatform::YouTube
    } else if host == "vimeo.com" || host.ends_with(".vimeo.com") {
        VideoPlatform::Vimeo
    } else if host == "dailymotion.com" || host.ends_with(".dailymotion.com") || host == "dai.ly" {
        VideoPlatform::Dailymotion
    } else if host == "tiktok.com" || host.ends_with(".tiktok.com") {
        VideoPlatform::TikTok
    } else if host == "twitter.com" || host == "x.com" || host.ends_with(".x.com") {
        VideoPlatform::TwitterX
    } else if host == "reddit.com" || host.ends_with(".reddit.com") || host == "v.redd.it" {
        VideoPlatform::Reddit
    } else {
        VideoPlatform::GenericWeb(host.to_string())
    }
}

/// Reconstruit une URL propre en supprimant le pistage et en normalisant les raccourcis.
fn canonicalize_url(parsed: &Url, platform: &VideoPlatform, host: &str) -> String {
    let mut clean_url = parsed.clone();

    // 1. Cas particulier YouTube : conversion des raccourcis youtu.be/<id> et /shorts/<id>
    if *platform == VideoPlatform::YouTube {
        if host == "youtu.be" {
            let video_id = parsed.path().trim_start_matches('/');
            if !video_id.is_empty() {
                let mut new_url = Url::parse("https://www.youtube.com/watch").unwrap();
                new_url.query_pairs_mut().append_pair("v", video_id);

                // Préservation du timestamp si présent (?t=123)
                for (key, val) in parsed.query_pairs() {
                    if key == "t" {
                        new_url.query_pairs_mut().append_pair("t", &val);
                    }
                }
                return new_url.to_string();
            }
        } else if parsed.path().starts_with("/shorts/") {
            let video_id = parsed
                .path()
                .trim_start_matches("/shorts/")
                .trim_end_matches('/');
            if !video_id.is_empty() {
                let mut new_url = Url::parse("https://www.youtube.com/watch").unwrap();
                new_url.query_pairs_mut().append_pair("v", video_id);
                return new_url.to_string();
            }
        }
    }

    // 2. Nettoyage des paramètres de pistage publicitaire
    if parsed.query().is_some() {
        let filtered_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.to_lowercase().as_str()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        clean_url.set_query(None);
        if !filtered_pairs.is_empty() {
            let mut pairs_mut = clean_url.query_pairs_mut();
            for (k, v) in filtered_pairs {
                pairs_mut.append_pair(&k, &v);
            }
        }
    }

    clean_url.to_string()
}

/// Vérifie si une adresse IPv4 est privée, de bouclage ou réservée.
fn is_private_or_local_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
}

/// Vérifie si une adresse IPv6 est locale ou réservée.
fn is_private_or_local_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_control_chars() {
        assert!(validate_and_sanitize_url("").is_err());
        assert!(validate_and_sanitize_url("   ").is_err());
        assert!(validate_and_sanitize_url("https://example.com/test\0video").is_err());
    }

    #[test]
    fn test_disallowed_schemes() {
        assert!(validate_and_sanitize_url("file:///etc/passwd").is_err());
        assert!(validate_and_sanitize_url("javascript:alert(1)").is_err());
        assert!(validate_and_sanitize_url("data:text/html,test").is_err());
        assert!(validate_and_sanitize_url("ftp://example.com/video.mp4").is_err());
    }

    #[test]
    fn test_ssrf_protection() {
        assert!(validate_and_sanitize_url("http://localhost:8080/video").is_err());
        assert!(validate_and_sanitize_url("http://127.0.0.1/video.mp4").is_err());
        assert!(validate_and_sanitize_url("http://192.168.1.10/video.mp4").is_err());
        assert!(validate_and_sanitize_url("http://10.0.0.1/video.mp4").is_err());
    }

    #[test]
    fn test_youtube_normalization_short_url() {
        let validated = validate_and_sanitize_url("https://youtu.be/dQw4w9WgXcQ?si=abcdef123&t=42")
            .expect("Validation réussie");
        assert_eq!(validated.platform, VideoPlatform::YouTube);
        assert_eq!(
            validated.canonical_url,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42"
        );
    }

    #[test]
    fn test_youtube_shorts_normalization() {
        let validated =
            validate_and_sanitize_url("https://www.youtube.com/shorts/abcdef12345?feature=share")
                .expect("Validation réussie");
        assert_eq!(validated.platform, VideoPlatform::YouTube);
        assert_eq!(
            validated.canonical_url,
            "https://www.youtube.com/watch?v=abcdef12345"
        );
    }

    #[test]
    fn test_tracking_params_removal() {
        let validated = validate_and_sanitize_url("https://cdn.example.org/watch/12345?utm_source=newsletter&utm_medium=email&fbclid=XYZ&quality=hd")
            .expect("Validation réussie");
        assert_eq!(
            validated.platform,
            VideoPlatform::GenericWeb("cdn.example.org".to_string())
        );
        assert!(validated.canonical_url.contains("quality=hd"));
        assert!(!validated.canonical_url.contains("utm_source"));
        assert!(!validated.canonical_url.contains("fbclid"));
    }

    #[test]
    fn test_generic_valid_video_url() {
        let validated = validate_and_sanitize_url("https://cdn.example.org/videos/nature.mp4")
            .expect("Validation réussie");
        assert_eq!(
            validated.platform,
            VideoPlatform::GenericWeb("cdn.example.org".to_string())
        );
        assert_eq!(validated.domain, "cdn.example.org");
    }
}
