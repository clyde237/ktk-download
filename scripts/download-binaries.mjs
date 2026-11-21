#!/usr/bin/env node

/**
 * Script de téléchargement des dépendances tierces (yt-dlp & ffmpeg)
 * pour l'empaquetage de l'installateur Windows (NSIS) de ktk-download.
 */

import fs from 'node:fs';
import path from 'node:path';
import { pipeline } from 'node:stream/promises';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(__dirname, '..');
const TAURI_BIN_DIR = path.join(ROOT_DIR, 'src-tauri', 'binaries');

const YTDLP_WINDOWS_URL = 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe';
const FFMPEG_WINDOWS_URL =
  'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip';

/**
 * Télécharge un fichier depuis une URL avec suivi de progression.
 */
async function downloadFile(url, destPath) {
  console.log(`Téléchargement de ${url} vers ${destPath}...`);
  const response = await fetch(url, {
    headers: { 'User-Agent': 'ktk-download-packager/1.0' },
    redirect: 'follow'
  });

  if (!response.ok) {
    throw new Error(`Échec HTTP ${response.status} pour ${url}`);
  }

  const fileStream = fs.createWriteStream(destPath);
  await pipeline(response.body, fileStream);
  console.log(`✓ Téléchargé : ${path.basename(destPath)}`);
}

/**
 * Télécharge et prépare les binaires pour Windows.
 */
async function prepareWindowsBinaries(force = false) {
  fs.mkdirSync(TAURI_BIN_DIR, { recursive: true });

  const ytdlpDest = path.join(TAURI_BIN_DIR, 'yt-dlp.exe');
  const ffmpegDest = path.join(TAURI_BIN_DIR, 'ffmpeg.exe');
  const ffprobeDest = path.join(TAURI_BIN_DIR, 'ffprobe.exe');

  // 1. yt-dlp.exe
  if (!force && fs.existsSync(ytdlpDest) && fs.statSync(ytdlpDest).size > 5_000_000) {
    console.log(`✓ yt-dlp.exe déjà présent dans ${TAURI_BIN_DIR}`);
  } else {
    await downloadFile(YTDLP_WINDOWS_URL, ytdlpDest);
  }

  // 2. ffmpeg.exe & ffprobe.exe
  if (
    !force &&
    fs.existsSync(ffmpegDest) &&
    fs.statSync(ffmpegDest).size > 10_000_000 &&
    fs.existsSync(ffprobeDest)
  ) {
    console.log(`✓ ffmpeg.exe et ffprobe.exe déjà présents dans ${TAURI_BIN_DIR}`);
  } else {
    const tempZip = path.join(TAURI_BIN_DIR, 'ffmpeg-temp.zip');
    const tempExtract = path.join(TAURI_BIN_DIR, 'ffmpeg-extracted');

    try {
      await downloadFile(FFMPEG_WINDOWS_URL, tempZip);
      fs.mkdirSync(tempExtract, { recursive: true });

      console.log('Extraction des binaires ffmpeg...');
      // Utilisation de tar disponible sous Linux, macOS et Windows 10+
      try {
        execSync(`tar -xf "${tempZip}" -C "${tempExtract}"`, { stdio: 'inherit' });
      } catch (_e) {
        // Fallback PowerShell sous Windows si tar n'est pas disponible
        if (process.platform === 'win32') {
          execSync(
            `powershell -Command "Expand-Archive -Path '${tempZip}' -DestinationPath '${tempExtract}' -Force"`,
            { stdio: 'inherit' }
          );
        } else {
          execSync(`unzip -q -o "${tempZip}" -d "${tempExtract}"`, { stdio: 'inherit' });
        }
      }

      // Recherche récursive des exécutables extraits
      function findFile(dir, fileName) {
        const entries = fs.readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
          const fullPath = path.join(dir, entry.name);
          if (entry.isDirectory()) {
            const found = findFile(fullPath, fileName);
            if (found) return found;
          } else if (entry.name.toLowerCase() === fileName.toLowerCase()) {
            return fullPath;
          }
        }
        return null;
      }

      const extractedFfmpeg = findFile(tempExtract, 'ffmpeg.exe');
      const extractedFfprobe = findFile(tempExtract, 'ffprobe.exe');

      if (!extractedFfmpeg) {
        throw new Error('ffmpeg.exe introuvable dans l’archive téléchargée.');
      }

      fs.copyFileSync(extractedFfmpeg, ffmpegDest);
      console.log(`✓ ffmpeg.exe installé dans ${TAURI_BIN_DIR}`);

      if (extractedFfprobe) {
        fs.copyFileSync(extractedFfprobe, ffprobeDest);
        console.log(`✓ ffprobe.exe installé dans ${TAURI_BIN_DIR}`);
      }
    } finally {
      if (fs.existsSync(tempZip)) fs.unlinkSync(tempZip);
      if (fs.existsSync(tempExtract)) {
        fs.rmSync(tempExtract, { recursive: true, force: true });
      }
    }
  }

  console.log('\n🎉 Binaires Windows prêts pour le packaging :');
  console.log(
    ` - yt-dlp : ${ytdlpDest} (${(fs.statSync(ytdlpDest).size / 1024 / 1024).toFixed(1)} Mo)`
  );
  console.log(
    ` - ffmpeg : ${ffmpegDest} (${(fs.statSync(ffmpegDest).size / 1024 / 1024).toFixed(1)} Mo)`
  );
  if (fs.existsSync(ffprobeDest)) {
    console.log(
      ` - ffprobe : ${ffprobeDest} (${(fs.statSync(ffprobeDest).size / 1024 / 1024).toFixed(1)} Mo)`
    );
  }
}

const args = process.argv.slice(2);
const force = args.includes('--force');

prepareWindowsBinaries(force).catch((err) => {
  console.error('Erreur lors de la préparation des binaires :', err);
  process.exit(1);
});
