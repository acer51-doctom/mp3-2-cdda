// No explicit macro import needed; rely on #[macro_use] in main.rs

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use std::io::Read;

pub fn convert_files(paths: Vec<PathBuf>, cancel_flag: Arc<Mutex<bool>>) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    paths.into_iter().for_each(|path| {
        if *cancel_flag.lock().unwrap() {
            log_info!("Conversion cancelled by user at process level.");
            return;
        }

        let files_to_process = if path.is_dir() {
            log_info!("Processing folder: {:?}", path);
            let mut files = Vec::new();
            for entry in std::fs::read_dir(&path)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_ok_and(|ft| ft.is_file()))
            {
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("mp3") {
                    files.push(file_path);
                }
            }
            files
        } else if path.extension().and_then(|s| s.to_str()) == Some("mp3") {
            log_info!("Processing single file: {:?}", path);
            vec![path]
        } else {
            log_warn!("Skipping non-MP3 file or directory: {:?}", path);
            return;
        };

        if files_to_process.is_empty() {
            log_warn!("No MP3 files found in {:?}", path);
            return;
        }

        let parent_folder = path.parent().unwrap_or_else(|| Path::new("."));
        let output_folder = parent_folder.join("CDDA_Converted");
        if let Err(e) = fs::create_dir_all(&output_folder) {
            log_error!("Failed to create output directory: {:?}", e);
            return;
        }

        for file_path in files_to_process {
            if *cancel_flag.lock().unwrap() {
                log_info!("Conversion cancelled by user before processing file: {:?}", file_path);
                return;
            }

            let start_time = std::time::Instant::now();
            log_info!("Starting conversion of: {:?}", file_path);
            if let Err(e) = convert_with_ffmpeg(&file_path, &output_folder, &cancel_flag) {
                log_error!("Failed to convert {}: {:?}", file_path.display(), e);
            } else {
                log_info!("Conversion completed in {:.2}s: {:?}", start_time.elapsed().as_secs_f32(), file_path);
            }
        }
    });

    log_info!("Conversion process complete!");
    Ok(())
}

fn convert_with_ffmpeg(input_path: &Path, output_dir: &Path, cancel_flag: &Arc<Mutex<bool>>) -> Result<()> {
    let output_filename = input_path
        .file_stem()
        .ok_or_else(|| anyhow::anyhow!("Invalid input filename"))?
        .to_string_lossy()
        .into_owned() + ".wav";
    let output_path = output_dir.join(output_filename);

    log_info!("Initiating ffmpeg conversion for: {:?}", input_path);
    let mut child = Command::new("ffmpeg")
        .args(&[
            "-i", input_path.to_str().unwrap(),
            "-acodec", "pcm_s16le",
            "-ac", "2",
            "-ar", "44100",
            "-y", // Overwrite output files without asking
            output_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn ffmpeg process")?;

    let mut progress_logged = false;
    while !child.try_wait()?.is_some() {
        if *cancel_flag.lock().unwrap() {
            log_info!("Cancelling ffmpeg process for: {:?}", input_path);
            child.kill().context("Failed to kill ffmpeg process")?;
            return Err(anyhow::anyhow!("Conversion cancelled for {:?}", input_path));
        }

        if let Some(stderr) = child.stderr.as_mut() {
            let mut buffer = [0; 1024];
            if let Ok(bytes_read) = stderr.read(&mut buffer) {
                if bytes_read > 0 {
                    let output = String::from_utf8_lossy(&buffer[..bytes_read]);
                    if output.contains("time=") && !progress_logged {
                        log_info!("ffmpeg progress for {:?}: {}", input_path, output);
                        progress_logged = true; // Log progress once to avoid spam
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100)); // Poll every 100ms
    }

    let status = child.wait().context("Failed to wait for ffmpeg process")?;
    if !status.success() {
        let stderr = child.stderr.and_then(|mut s| {
            let mut buf = String::new();
            use std::io::Read;
            let _ = s.read_to_string(&mut buf);
            Some(buf)
        }).unwrap_or_else(|| "No stderr available".to_string());
        return Err(anyhow::anyhow!("ffmpeg failed: {}", stderr));
    }

    log_debug!("ffmpeg output for {:?}: {:?}", input_path, child.stdout);
    Ok(())
}