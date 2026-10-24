use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ktk_download_lib::commands::{
    cancel_download, download_video, inspect_url, pause_download, validate_url,
};
use ktk_download_lib::downloader::DownloadRegistry;
use ktk_download_lib::error::CommandError;
use ktk_download_lib::models::{DownloadProgressPayload, DownloadRequest, DownloadStatus};
use tauri::{Listener, Manager};

/// Crée un script mock temporaire pour simuler yt-dlp de manière déterministe.
fn create_mock_script(content: &str) -> PathBuf {
    let unique = format!("ktk-mock-ipc-{}.sh", uuid::Uuid::new_v4());
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

/// Mutex global pour sérialiser les tests modifiant la variable d'environnement `YTDLP_PATH`.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_ipc_validate_url_roundtrip() {
    // 1. URL YouTube standard
    let valid_yt = "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string();
    let res = validate_url(valid_yt).expect("Validation YouTube");
    assert!(res.is_valid);
    assert_eq!(res.platform, "YouTube");
    assert_eq!(res.domain, "www.youtube.com");

    // 2. URL flux direct MP4
    let valid_mp4 = "https://cdn.example.org/videos/clip_720p.mp4".to_string();
    let res = validate_url(valid_mp4).expect("Validation flux direct");
    assert!(res.is_valid);
    assert_eq!(res.domain, "cdn.example.org");

    // 3. URLs invalides
    assert!(validate_url("not-a-valid-url".to_string()).is_err());
    assert!(validate_url("ftp://example.com/video.mp4".to_string()).is_err());
    assert!(validate_url("   ".to_string()).is_err());
}

#[test]
fn test_ipc_inspect_url_roundtrip() {
    let _guard = ENV_LOCK.lock().unwrap();

    let script = r#"#!/bin/sh
cat << 'EOF'
{
  "id": "ipc_meta_test",
  "title": "Vidéo Documentaire Nature",
  "description": "Documentaire animalier 4K",
  "uploader": "Faune & Flore",
  "thumbnail": "https://example.com/nature.jpg",
  "duration": 360,
  "extractor": "youtube",
  "formats": [
    {
      "format_id": "137",
      "ext": "mp4",
      "height": 1080,
      "width": 1920,
      "vcodec": "avc1.640028",
      "acodec": "none",
      "filesize": 104857600
    },
    {
      "format_id": "140",
      "ext": "m4a",
      "vcodec": "none",
      "acodec": "mp4a.40.2",
      "filesize": 10485760
    }
  ]
}
EOF
exit 0
"#;
    let mock_bin = create_mock_script(script);
    unsafe {
        std::env::set_var("YTDLP_PATH", &mock_bin);
    }

    tauri::async_runtime::block_on(async {
        let meta = inspect_url("https://www.youtube.com/watch?v=ipc_meta_test".to_string())
            .await
            .expect("Analyse IPC inspect_url");

        assert_eq!(meta.id, "ipc_meta_test");
        assert_eq!(meta.title, "Vidéo Documentaire Nature");
        assert_eq!(meta.author.as_deref(), Some("Faune & Flore"));
        assert_eq!(meta.duration_seconds, Some(360));
        assert_eq!(meta.source_platform, "YouTube");
        assert!(!meta.available_formats.is_empty());

        let has_1080p = meta
            .available_formats
            .iter()
            .any(|f| f.format_id == "1080p");
        assert!(
            has_1080p,
            "Format 1080p doit être généré à partir de height: 1080"
        );
    });

    unsafe {
        std::env::remove_var("YTDLP_PATH");
    }
    let _ = std::fs::remove_file(&mock_bin);
}

#[test]
fn test_ipc_full_download_roundtrip() {
    let _guard = ENV_LOCK.lock().unwrap();

    let script = r#"#!/bin/sh
echo "ktk: 25.0%| 2.00MiB/s|00:30|2097152"
echo "ktk: 50.0%| 2.50MiB/s|00:20|2097152"
echo "ktk: 75.0%| 3.00MiB/s|00:10|2097152"
echo "ktk:100.0%| 3.50MiB/s|00:00|2097152"

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
echo "Contenu vidéo final round-trip IPC" > "$ACTUAL_FILE"
echo "ktk-file:$ACTUAL_FILE"
exit 0
"#;
    let mock_bin = create_mock_script(script);
    unsafe {
        std::env::set_var("YTDLP_PATH", &mock_bin);
    }

    let app = Box::leak(Box::new(tauri::test::mock_app()));
    app.manage(DownloadRegistry::default());

    let target_dir = std::env::temp_dir().join(format!("ktk-ipc-full-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&target_dir).unwrap();

    let progress_events = Arc::new(Mutex::new(Vec::<DownloadProgressPayload>::new()));
    let events_clone = Arc::clone(&progress_events);

    // Écoute de l'événement IPC Tauri "download-progress"
    app.listen("download-progress", move |event| {
        if let Ok(payload) = serde_json::from_str::<DownloadProgressPayload>(event.payload()) {
            events_clone.lock().unwrap().push(payload);
        }
    });

    let download_id = uuid::Uuid::new_v4().to_string();
    let request = DownloadRequest {
        url: "https://www.youtube.com/watch?v=full_roundtrip".to_string(),
        format_id: Some("best".to_string()),
        output_dir: Some(target_dir.to_string_lossy().to_string()),
        custom_filename: Some("MonFilmComplet".to_string()),
        download_id: Some(download_id.clone()),
    };

    tauri::async_runtime::block_on(async {
        let registry = app.state::<DownloadRegistry>();
        let result = download_video(app.handle().clone(), registry, request)
            .await
            .expect("Round-trip download_video réussi");

        assert_eq!(result.download_id, download_id);
        assert!(result.file_size_bytes > 0);
        let final_path = PathBuf::from(&result.file_path);
        assert!(final_path.exists());

        // Vérification des événements reçus lors du cycle de vie
        let received = progress_events.lock().unwrap().clone();
        assert!(
            !received.is_empty(),
            "Au moins un événement de progression attendu"
        );

        // Premier événement : Queued
        assert_eq!(received[0].status, DownloadStatus::Queued);
        assert_eq!(received[0].download_id, download_id);

        // Dernier événement : Completed avec 100%
        let last = received.last().unwrap();
        assert_eq!(last.status, DownloadStatus::Completed);
        assert_eq!(last.progress_percentage, 100.0);

        let _ = std::fs::remove_file(&final_path);
        let _ = std::fs::remove_dir_all(&target_dir);
    });

    unsafe {
        std::env::remove_var("YTDLP_PATH");
    }
    let _ = std::fs::remove_file(&mock_bin);
}

#[test]
fn test_ipc_cancel_download_roundtrip() {
    let _guard = ENV_LOCK.lock().unwrap();

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
echo "ktk: 12.0%| 1.00MiB/s|01:30|10485760"

while true; do
  sleep 0.05
done
"#;
    let mock_bin = create_mock_script(script);
    unsafe {
        std::env::set_var("YTDLP_PATH", &mock_bin);
    }

    let app = Box::leak(Box::new(tauri::test::mock_app()));
    app.manage(DownloadRegistry::default());

    let target_dir = std::env::temp_dir().join(format!("ktk-ipc-cancel-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&target_dir).unwrap();

    let download_id = uuid::Uuid::new_v4().to_string();
    let request = DownloadRequest {
        url: "https://www.youtube.com/watch?v=cancel_roundtrip".to_string(),
        format_id: Some("best".to_string()),
        output_dir: Some(target_dir.to_string_lossy().to_string()),
        custom_filename: Some("FilmAnnule".to_string()),
        download_id: Some(download_id.clone()),
    };

    let cancelled_event_received = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&cancelled_event_received);

    app.listen("download-progress", move |event| {
        if let Ok(payload) = serde_json::from_str::<DownloadProgressPayload>(event.payload())
            && payload.status == DownloadStatus::Cancelled
        {
            flag_clone.store(true, Ordering::SeqCst);
        }
    });

    tauri::async_runtime::block_on(async {
        let app_handle = app.handle().clone();
        let registry = app.state::<DownloadRegistry>();
        let dl_id_clone = download_id.clone();

        let download_task =
            tokio::spawn(async move { download_video(app_handle, registry, request).await });

        // Laisser le processus démarrer
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Appel IPC cancel_download
        let registry_cancel = app.state::<DownloadRegistry>();
        let cancelled = cancel_download(registry_cancel, dl_id_clone)
            .await
            .expect("Appel cancel_download");
        assert!(
            cancelled,
            "cancel_download doit renvoyer true pour un téléchargement actif"
        );

        let result = download_task.await.expect("Join download_task");
        assert!(matches!(result, Err(CommandError::DownloadCancelled)));

        // Vérifier l'événement IPC émis
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(cancelled_event_received.load(Ordering::SeqCst));

        // Vérifier que le dossier ne contient plus aucun fichier partiel résiduel
        let files = std::fs::read_dir(&target_dir).unwrap().count();
        assert_eq!(
            files, 0,
            "Les fichiers partiels doivent être supprimés après annulation"
        );

        let _ = std::fs::remove_dir_all(&target_dir);
    });

    unsafe {
        std::env::remove_var("YTDLP_PATH");
    }
    let _ = std::fs::remove_file(&mock_bin);
}

#[test]
fn test_ipc_pause_and_resume_roundtrip() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pause_script = r#"#!/bin/sh
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
echo "donnees partielles" > "$PART_FILE"
echo "ktk: 35.0%| 1.00MiB/s|00:40|10485760"

while true; do
  sleep 0.05
done
"#;
    let mock_bin_pause = create_mock_script(pause_script);
    unsafe {
        std::env::set_var("YTDLP_PATH", &mock_bin_pause);
    }

    let app = Box::leak(Box::new(tauri::test::mock_app()));
    app.manage(DownloadRegistry::default());

    let target_dir = std::env::temp_dir().join(format!("ktk-ipc-pause-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&target_dir).unwrap();

    let download_id = uuid::Uuid::new_v4().to_string();
    let request = DownloadRequest {
        url: "https://www.youtube.com/watch?v=pause_resume".to_string(),
        format_id: Some("best".to_string()),
        output_dir: Some(target_dir.to_string_lossy().to_string()),
        custom_filename: Some("FilmEnPause".to_string()),
        download_id: Some(download_id.clone()),
    };

    tauri::async_runtime::block_on(async {
        let app_handle = app.handle().clone();
        let registry = app.state::<DownloadRegistry>();
        let dl_id_clone = download_id.clone();
        let req_clone = request.clone();

        let download_task =
            tokio::spawn(async move { download_video(app_handle, registry, req_clone).await });

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Appel IPC pause_download
        let registry_pause = app.state::<DownloadRegistry>();
        let paused = pause_download(registry_pause, dl_id_clone).expect("Appel pause_download");
        assert!(paused, "pause_download doit renvoyer true");

        let result = download_task.await.expect("Join download_task");
        assert!(matches!(result, Err(CommandError::DownloadPaused)));

        // Vérifier que le fichier partiel .part existe toujours !
        let part_exists = std::fs::read_dir(&target_dir)
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".part"));
        assert!(
            part_exists,
            "Le fichier partiel .part DOIT être conservé après une pause"
        );

        // Nettoyer premier mock
        let _ = std::fs::remove_file(&mock_bin_pause);

        // Étape 2 : Reprise avec un mock qui termine le téléchargement
        let resume_script = r#"#!/bin/sh
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
echo "Contenu vidéo final complété après reprise" > "$ACTUAL_FILE"
echo "ktk-file:$ACTUAL_FILE"
exit 0
"#;
        let mock_bin_resume = create_mock_script(resume_script);
        unsafe {
            std::env::set_var("YTDLP_PATH", &mock_bin_resume);
        }

        // Relance de download_video avec le même identifiant pour reprendre
        let registry_resume = app.state::<DownloadRegistry>();
        let resumed_res = download_video(app.handle().clone(), registry_resume, request)
            .await
            .expect("Reprise réussie");

        assert_eq!(resumed_res.download_id, download_id);
        assert!(Path::new(&resumed_res.file_path).exists());

        let _ = std::fs::remove_file(&resumed_res.file_path);
        let _ = std::fs::remove_dir_all(&target_dir);
        let _ = std::fs::remove_file(&mock_bin_resume);
    });

    unsafe {
        std::env::remove_var("YTDLP_PATH");
    }
}

#[test]
fn test_ipc_invalid_inputs_roundtrip() {
    let app = Box::leak(Box::new(tauri::test::mock_app()));
    app.manage(DownloadRegistry::default());

    tauri::async_runtime::block_on(async {
        // 1. Schéma d'URL non autorisé
        let req_bad_url = DownloadRequest {
            url: "ftp://example.com/video.mp4".to_string(),
            format_id: Some("best".to_string()),
            output_dir: None,
            custom_filename: None,
            download_id: None,
        };
        let err = download_video(
            app.handle().clone(),
            app.state::<DownloadRegistry>(),
            req_bad_url,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CommandError::InvalidUrl(_)));

        // 2. Dossier de destination relatif ou inexistant
        let req_bad_dir = DownloadRequest {
            url: "https://example.com/video.mp4".to_string(),
            format_id: Some("best".to_string()),
            output_dir: Some("../dossier_relatif_interdit".to_string()),
            custom_filename: None,
            download_id: None,
        };
        let err = download_video(
            app.handle().clone(),
            app.state::<DownloadRegistry>(),
            req_bad_dir,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CommandError::FileSystemError(_)));

        // 3. Préréglage de qualité inconnu
        let req_bad_quality = DownloadRequest {
            url: "https://example.com/video.mp4".to_string(),
            format_id: Some("8k_ultra_hd_inconnu".to_string()),
            output_dir: None,
            custom_filename: None,
            download_id: None,
        };
        let err = download_video(
            app.handle().clone(),
            app.state::<DownloadRegistry>(),
            req_bad_quality,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CommandError::DownloadFailed(_)));
    });
}
