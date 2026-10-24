use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Port d'écoute du serveur local dédié à la communication avec l'extension navigateur.
pub const EXTENSION_SERVER_PORT: u16 = 42124;

/// Démarre le serveur local de pont pour l'extension de navigateur.
///
/// Ce serveur n'écoute que sur l'adresse loopback `127.0.0.1:42124` afin de garantir
/// une sécurité absolue et d'empêcher tout accès depuis le réseau extérieur.
pub fn start_extension_server(app: AppHandle) {
    let app_handle = Arc::new(app);
    std::thread::Builder::new()
        .name("extension-server".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "Impossible de créer le runtime Tokio pour l'extension : {:?}",
                        e
                    );
                    return;
                }
            };

            rt.block_on(async move {
                let addr = SocketAddr::from(([127, 0, 0, 1], EXTENSION_SERVER_PORT));
                let listener = match TcpListener::bind(addr).await {
                    Ok(l) => {
                        log::info!("Serveur de pont d'extension actif sur http://{}", addr);
                        l
                    }
                    Err(e) => {
                        log::warn!(
                            "Impossible de lier le serveur d'extension sur {} ({:?})",
                            addr,
                            e
                        );
                        return;
                    }
                };

                loop {
                    match listener.accept().await {
                        Ok((socket, _)) => {
                            let app_clone = Arc::clone(&app_handle);
                            tokio::spawn(async move {
                                if let Err(e) = handle_client(socket, app_clone).await {
                                    log::debug!("Erreur traitement client extension : {:?}", e);
                                }
                            });
                        }
                        Err(e) => {
                            log::debug!("Échec d'acceptation de connexion : {:?}", e);
                        }
                    }
                }
            });
        })
        .expect("Échec du démarrage du thread serveur d'extension");
}

/// Requête reçue depuis l'extension de navigateur pour lancer un téléchargement.
///
/// Qualité et dossier sont choisis dans l'extension ; le backend les revérifie au lancement
/// (`QualityPreset::parse`, `validate_output_dir`).
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ExtensionDownloadPayload {
    pub url: String,
    pub title: Option<String>,
    pub format_id: Option<String>,
    pub output_dir: Option<String>,
}

/// Dossiers récemment utilisés dans l'application, proposés par l'extension.
#[derive(Debug, Default)]
pub struct RecentFolders(pub std::sync::Mutex<Vec<String>>);

/// Préfixes d'origine des extensions de navigateur (Firefox, Chromium, Safari).
const EXTENSION_ORIGIN_PREFIXES: &[&str] = &[
    "moz-extension://",
    "chrome-extension://",
    "safari-web-extension://",
];

/// Seule l'extension peut piloter le serveur. Une page web ne peut pas falsifier l'en-tête
/// `Origin` : sans ce filtre, n'importe quel site ouvert pourrait lancer des téléchargements ou
/// lire les dossiers de l'utilisateur. Sans `Origin`, la requête vient d'un programme local.
fn is_allowed_origin(origin: Option<&str>) -> bool {
    origin.is_none_or(|o| EXTENSION_ORIGIN_PREFIXES.iter().any(|p| o.starts_with(p)))
}

fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request
        .split("\r\n\r\n")
        .next()?
        .lines()
        .skip(1)
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case(name)
                .then_some(value.trim())
        })
}

async fn handle_client(
    mut socket: TcpStream,
    app: Arc<AppHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0u8; 8192];
    let bytes_read = socket.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return Ok(()),
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let origin = header_value(&request_str, "origin");
    if !is_allowed_origin(origin) {
        let body = r#"{"error":"Origine non autorisée"}"#;
        let response = format!(
            "HTTP/1.1 403 Forbidden\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // En-têtes CORS limités à l'extension qui a fait la requête
    let cors_headers = format!(
        "Access-Control-Allow-Origin: {}\r\n\
         Vary: Origin\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n",
        origin.unwrap_or("null")
    );

    if method == "OPTIONS" {
        let response = format!(
            "HTTP/1.1 204 No Content\r\n\
             {}\
             Content-Length: 0\r\n\
             Connection: close\r\n\r\n",
            cors_headers
        );
        socket.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    if method == "GET" && path == "/ping" {
        let body = r#"{"status":"ok","app":"ktk-download","version":"0.1.0"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             {}\
             Content-Type: application/json; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            cors_headers,
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    if method == "POST" && (path == "/download" || path == "/add") {
        // Extraction du corps JSON
        let body_start = request_str.find("\r\n\r\n").map(|idx| idx + 4).unwrap_or(0);
        let raw_body = &request_str[body_start..];

        if let Ok(payload) = serde_json::from_str::<ExtensionDownloadPayload>(raw_body) {
            log::info!("Téléchargement reçu depuis l'extension : {:?}", payload);
            // La fenêtre principale reste en arrière-plan : la popup du téléchargement s'ouvre.
            let _ = app.emit("extension-download", payload);

            let body = r#"{"success":true,"message":"Téléchargement transmis à ktk-download"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 {}\
                 Content-Type: application/json; charset=utf-8\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                cors_headers,
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await?;
            return Ok(());
        } else {
            let body = r#"{"success":false,"error":"Corps de requête JSON invalide"}"#;
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\n\
                 {}\
                 Content-Type: application/json; charset=utf-8\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                cors_headers,
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await?;
            return Ok(());
        }
    }

    if method == "GET" && path == "/folders" {
        let default_folder = dirs::download_dir().map(|d| d.to_string_lossy().to_string());
        let recent = app
            .try_state::<RecentFolders>()
            .map(|state| state.0.lock().unwrap().clone())
            .unwrap_or_default();
        let body = serde_json::json!({ "default": default_folder, "recent": recent }).to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             {}\
             Content-Type: application/json; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            cors_headers,
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Réponse 404 par défaut
    let body = r#"{"error":"Not Found"}"#;
    let response = format!(
        "HTTP/1.1 404 Not Found\r\n\
         {}\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        cors_headers,
        body.len(),
        body
    );
    socket.write_all(response.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_payload_deserialization() {
        let json = r#"{"url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","title":"Rick Astley","format_id":"1080p"}"#;
        let payload: ExtensionDownloadPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(payload.title.as_deref(), Some("Rick Astley"));
        assert_eq!(payload.format_id.as_deref(), Some("1080p"));
    }

    #[test]
    fn test_only_extensions_and_local_tools_are_allowed() {
        assert!(is_allowed_origin(None));
        assert!(is_allowed_origin(Some("moz-extension://4f2a-1c")));
        assert!(is_allowed_origin(Some("chrome-extension://abcdef")));
        assert!(!is_allowed_origin(Some("https://evil.example")));
        assert!(!is_allowed_origin(Some("null")));
    }

    #[test]
    fn test_header_value_is_case_insensitive_and_ignores_body() {
        let request = "POST /download HTTP/1.1\r\nHost: 127.0.0.1\r\nORIGIN: moz-extension://x\r\n\r\n{\"origin\":\"https://evil\"}";
        assert_eq!(header_value(request, "origin"), Some("moz-extension://x"));
        assert_eq!(header_value("GET /ping HTTP/1.1\r\n\r\n", "origin"), None);
    }

    #[test]
    fn test_extension_payload_optional_fields() {
        let json = r#"{"url":"https://example.com/video.mp4"}"#;
        let payload: ExtensionDownloadPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.url, "https://example.com/video.mp4");
        assert!(payload.title.is_none());
        assert!(payload.format_id.is_none());
    }
}
