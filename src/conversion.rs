use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::io::BufWriter;

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use rubato::{Resampler, SincFixedIn, WindowFunction, SincInterpolationParameters, SincInterpolationType};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;
use walkdir::WalkDir;

pub fn convert_files(paths: Vec<PathBuf>, cancel_flag: Arc<Mutex<bool>>) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    for path in paths {
        if *cancel_flag.lock().unwrap() {
            info!("Conversion cancelled by user.");
            break;
        }

        let files_to_process = if path.is_dir() {
            info!("Processing folder: {:?}", path);
            let mut files = Vec::new();
            for entry in WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("mp3") {
                    files.push(file_path.to_path_buf());
                }
            }
            files
        } else if path.extension().and_then(|s| s.to_str()) == Some("mp3") {
            info!("Processing single file: {:?}", path);
            vec![path.clone()] // Clone path to avoid move
        } else {
            warn!("Skipping non-MP3 file or directory: {:?}", path);
            continue;
        };

        if files_to_process.is_empty() {
            warn!("No MP3 files found in {:?}", path);
            continue;
        }

        let parent_folder = path.parent().unwrap_or_else(|| Path::new("."));
        let output_folder = parent_folder.join("CDDA_Converted");
        fs::create_dir_all(&output_folder)
            .context("Failed to create output directory")?;

        for file_path in files_to_process {
            if *cancel_flag.lock().unwrap() {
                info!("Conversion cancelled by user.");
                break;
            }

            info!("Starting conversion of: {:?}", file_path);
            if let Err(e) = process_file(&file_path, &output_folder, &cancel_flag) {
                error!("Failed to process {}: {:?}", file_path.display(), e);
            }
        }
    }

    info!("Conversion process complete!");
    Ok(())
}

fn process_file(
    input_path: &Path,
    output_dir: &Path,
    cancel_flag: &Arc<Mutex<bool>>,
) -> Result<()> {
    let file = File::open(input_path)
        .context("Failed to open input file")?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let probed = get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ).context("Failed to probe media format")?;

    let mut format = probed.format;
    let track = format.default_track()
        .ok_or_else(|| anyhow::anyhow!("No default track found"))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let output_filename = input_path
        .file_stem()
        .ok_or_else(|| anyhow::anyhow!("Invalid input filename"))?
        .to_string_lossy()
        .into_owned() + ".wav";
    let output_path = output_dir.join(output_filename);

    let mut writer = prepare_wav_writer(&output_path, 44100)
        .context("Failed to prepare WAV writer")?;

    let signal_spec = track.codec_params
        .sample_rate
        .map(|rate| SignalSpec::new(rate, track.codec_params.channels.unwrap_or_default()))
        .ok_or_else(|| anyhow::anyhow!("Missing sample rate"))?;

    info!("Decoding and converting: {:?}", input_path);
    while let Ok(packet) = format.next_packet() {
        if *cancel_flag.lock().unwrap() {
            info!("Conversion cancelled by user.");
            writer.finalize().context("Failed to finalize WAV file")?;
            return Ok(());
        }

        let decoded = decoder.decode(&packet)
            .context("Failed to decode packet")?;

        process_audio_buffer(decoded, signal_spec, &mut writer)
            .context("Failed to process audio buffer")?;
    }

    writer.finalize()
        .context("Failed to finalize WAV file")?;

    info!("Successfully converted: {:?}", input_path);
    Ok(())
}

fn prepare_wav_writer(path: &Path, sample_rate: u32) -> Result<hound::WavWriter<BufWriter<File>>> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let file = File::create(path)
        .context("Failed to create output file")?;
    let writer = hound::WavWriter::new(BufWriter::new(file), spec)
        .context("Failed to create WAV writer")?;
    Ok(writer)
}

fn process_audio_buffer(
    buffer: AudioBufferRef<'_>,
    signal_spec: SignalSpec,
    writer: &mut hound::WavWriter<BufWriter<File>>,
) -> Result<()> {
    let mut sample_buf = SampleBuffer::<i16>::new(buffer.capacity() as u64, signal_spec);
    sample_buf.copy_interleaved_ref(buffer);

    let target_rate = 44100;
    if signal_spec.rate != target_rate {
        debug!("Resampling from {} Hz to {} Hz", signal_spec.rate, target_rate);
        resample_audio(&sample_buf, signal_spec, target_rate, writer)?;
    } else {
        let samples = convert_to_stereo(&sample_buf, signal_spec.channels.count());
        for sample in samples {
            writer.write_sample(sample)
                .context("Failed to write sample")?;
        }
    }

    Ok(())
}

fn convert_to_stereo(buffer: &SampleBuffer<i16>, channels: usize) -> Vec<i16> {
    if channels == 1 {
        buffer.samples()
            .iter()
            .flat_map(|s| [*s, *s])
            .collect::<Vec<i16>>()
    } else {
        buffer.samples().to_vec()
    }
}

fn resample_audio(
    buffer: &SampleBuffer<i16>,
    signal_spec: SignalSpec,
    target_rate: u32,
    writer: &mut hound::WavWriter<BufWriter<File>>,
) -> Result<()> {
    let original_rate = signal_spec.rate;
    let channels = signal_spec.channels.count();
    let ratio = target_rate as f64 / original_rate as f64;

    let mut resampler = SincFixedIn::<f64>::new(
        ratio,
        2.0,
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
            interpolation: SincInterpolationType::Linear,
        },
        buffer.samples().len() / channels,
        channels,
    ).context("Failed to create resampler")?;

    let samples_f64: Vec<f64> = buffer.samples()
        .iter()
        .map(|s| f64::from(*s) / f64::from(i16::MAX))
        .collect();

    let input = if channels == 1 {
        vec![samples_f64]
    } else {
        let left = samples_f64.iter().step_by(2).copied().collect::<Vec<_>>();
        let right = samples_f64.iter().skip(1).step_by(2).copied().collect::<Vec<_>>();
        vec![left, right]
    };

    let resampled = resampler.process(&input, None)
        .context("Resampling failed")?;

    let resampled_i16: Vec<i16> = if channels == 1 {
        resampled[0]
            .iter()
            .flat_map(|s| [(s * f64::from(i16::MAX)).round() as i16; 2])
            .collect()
    } else {
        resampled[0]
            .iter()
            .zip(resampled[1].iter())
            .flat_map(|(l, r)| [(l * f64::from(i16::MAX)).round() as i16, (r * f64::from(i16::MAX)).round() as i16])
            .collect()
    };

    for sample in &resampled_i16 { // Use reference to avoid move
        writer.write_sample(*sample)
            .context("Failed to write sample")?;
    }

    debug!("Resampling completed for {} samples", resampled_i16.len());
    Ok(())
}