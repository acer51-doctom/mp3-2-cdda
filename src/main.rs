mod conversion;

use eframe::{egui, App, Frame};
use rfd::FileDialog;
use single_instance::SingleInstance;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use env_logger;

struct ConverterApp {
    selected_files: Vec<PathBuf>,
    is_processing: bool,
    progress_message: String,
    last_error: Option<String>,
    cancel_flag: Arc<Mutex<bool>>,
    instance_guard: SingleInstance,
}

impl Default for ConverterApp {
    fn default() -> Self {
        Self {
            selected_files: Vec::new(),
            is_processing: false,
            progress_message: "Ready to convert MP3 files to CDDA".to_string(),
            last_error: None,
            cancel_flag: Arc::new(Mutex::new(false)),
            instance_guard: SingleInstance::new("mp3_to_cdda_converter").unwrap(),
        }
    }
}

impl ConverterApp {
    fn select_files(&mut self) {
        if let Some(files) = FileDialog::new()
            .add_filter("MP3 Files", &["mp3"])
            .pick_files()
        {
            self.selected_files = files;
            self.progress_message = format!("Selected {} files", self.selected_files.len());
            self.last_error = None;
        }
    }

    fn start_conversion(&mut self) {
        if self.selected_files.is_empty() {
            self.last_error = Some("No files selected".to_string());
            return;
        }

        self.is_processing = true;
        self.progress_message = "Starting conversion...".to_string();
        *self.cancel_flag.lock().unwrap() = false;

        let files = self.selected_files.clone();
        let cancel_flag = Arc::clone(&self.cancel_flag);
        let status_sender = self.create_status_sender();

        thread::spawn(move || {
            if let Err(e) = conversion::convert_files(files, cancel_flag) {
                status_sender.send(Err(e.to_string())).ok();
            } else {
                status_sender.send(Ok("Conversion complete!".to_string())).ok();
            }
        });
    }

    fn create_status_sender(&self) -> std::sync::mpsc::Sender<Result<String, String>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let ctx = eframe::egui::Context::default();

        ctx.request_repaint();
        std::thread::spawn(move || {
            if let Ok(result) = receiver.recv() {
                ctx.request_repaint();
                match result {
                    Ok(msg) => println!("Success: {}", msg),
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
        });

        sender
    }
}

impl App for ConverterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MP3 to CDDA Converter");

            if let Some(err) = &self.last_error {
                ui.colored_label(egui::Color32::RED, err);
            }

            if !self.is_processing {
                self.show_file_selection(ui);
            } else {
                self.show_conversion_progress(ui);
            }

            ui.separator();
            ui.label(&self.progress_message);
        });

        if !self.instance_guard.is_single() {
            self.last_error = Some("Another instance is already running".to_string());
        }

        ctx.request_repaint();
    }
}

impl ConverterApp {
    fn show_file_selection(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            if ui.button("📁 Select MP3 Files").clicked() {
                self.select_files();
            }

            if !self.selected_files.is_empty() {
                ui.separator();
                ui.label("Selected files:");
                
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for file in &self.selected_files {
                            ui.label(file.file_name().unwrap().to_string_lossy());
                        }
                    });

                if ui.button("🔃 Convert to CDDA").clicked() {
                    self.start_conversion();
                }
            }
        });
    }

    fn show_conversion_progress(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add(egui::Spinner::new().size(40.0));
            ui.label("Converting files...");
            
            if ui.button("❌ Cancel").clicked() {
                *self.cancel_flag.lock().unwrap() = true;
                self.progress_message = "Cancelling...".to_string();
            }
        });
    }
}

fn main() {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 500.0])
            .with_min_inner_size([300.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MP3 to CDDA Converter",
        options,
        Box::new(|_cc| Box::new(ConverterApp::default())),
    );
}