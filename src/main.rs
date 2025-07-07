use eframe::{egui, epi};
use rfd::FileDialog;
use std::path::{PathBuf};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use single_instance::SingleInstance;

// Audio libs
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::default::{get_probe};
use std::fs::File;

struct App {
    selected_files: Vec<PathBuf>,
    status: String,
    is_working: bool,
    cancel_requested: Arc<Mutex<bool>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            selected_files: Vec::new(),
            status: "Select files to convert.".to_string(),
            is_working: false,
            cancel_requested: Arc::new(Mutex::new(false)),
        }
    }
}

impl epi::App for App {
    fn name(&self) -> &str {
        "MP3 to CDDA Converter"
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut epi::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MP3 to CDDA Converter");

            if !self.is_working {
                if ui.button("Select MP3 Files").clicked() {
                    if let Some(files) = FileDialog::new()
                        .add_filter("MP3 audio", &["mp3"])
                        .pick_files()
                    {
                        self.selected_files = files;
                        self.status = format!("Selected {} files.", self.selected_files.len());
                    }
                }

                if !self.selected_files.is_empty() {
                    if ui.button("Convert to CDDA").clicked() {
                        self.is_working = true;
                        self.status = "Starting conversion...".to_string();
                        *self.cancel_requested.lock().unwrap() = false;

                        let files = self.selected_files.clone();
                        let cancel_flag = Arc::clone(&self.cancel_requested);

                        // Run conversion in a separate thread so UI stays responsive.
                        thread::spawn(move || {
                            if let Err(e) = convert_files(&files, cancel_flag) {
                                println!("Conversion error: {}", e);
                            }
                        });
                    }
                }

                ui.label(&self.status);
            } else {
                ui.add(egui::Spinner::new());
                ui.label(egui::RichText::new("Working on your files... Hang tight!").strong());

                if ui.button("Cancel").clicked() {
                    *self.cancel_requested.lock().unwrap() = true;
                    self.status = "Cancel requested.".to_string();
                }
            }
        });

        ctx.request_repaint(); // Keep UI updating while working.
    }
}

fn main() {
    // Allow only one instance.
    let instance = SingleInstance::new("mp3_to_cdda_gui_instance").unwrap();
    if !instance.is_single() {
        println!("Another instance is already running.");
        return;
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "MP3 to CDDA Converter",
        options,
        Box::new(|_cc| Box::new(App::default())),
    );
}

fn convert_files(files: &Vec<PathBuf>, cancel_flag: Arc<Mutex<bool>>) -> anyhow::Result<()> {
    for file_path in files {
        if *cancel_flag.lock().unwrap() {
            println!("Conversion cancelled by user.");
            break;
        }

        println!("Processing: {:?}", file_path);

        let file = File::open(&file_path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let probed = get_probe().format(
            &FormatOptions::default(),
            &MetadataOptions::default(),
            mss,
            &Default::default(),
        )?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| anyhow::anyhow!("No default track found"))?;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;

        // Here you’d decode + resample + write WAV with `hound` + `rubato`.
        // For now, just sleep to simulate work.
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    println!("Conversion done.");
    Ok(())
}
