use anyhow::Result;
use av_metrics_decoders::{Decoder, FfmpegDecoder, Pixel};
use crossterm::tty::IsTty;
use image::ColorType;
use indicatif::{HumanDuration, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use num_traits::FromPrimitive;
use ssimulacra2::{
    compute_frame_ssimulacra2, ColorPrimaries, MatrixCoefficients, TransferCharacteristic, Yuv,
    YuvConfig,
};
use statrs::statistics::{Data, Distribution, Median, OrderStatistics};
use std::collections::BTreeMap;
use std::io::stderr;
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏  ";
const INDICATIF_PROGRESS_TEMPLATE: &str = if cfg!(windows) {
    // Do not use a spinner on Windows since the default console cannot display
    // the characters used for the spinner
    "{elapsed_precise:.bold} ▕{wide_bar:.blue/white.dim}▏ {percent:.bold}  {pos} ({fps:.bold}, eta {fixed_eta}{msg})"
} else {
    "{spinner:.green.bold} {elapsed_precise:.bold} ▕{wide_bar:.blue/white.dim}▏ {percent:.bold}  {pos} ({fps:.bold}, eta {fixed_eta}{msg})"
};
const INDICATIF_SPINNER_TEMPLATE: &str = if cfg!(windows) {
    // Do not use a spinner on Windows since the default console cannot display
    // the characters used for the spinner
    "{elapsed_precise:.bold} [{wide_bar:.blue/white.dim}]  {pos} frames ({fps:.bold})"
} else {
    "{spinner:.green.bold} {elapsed_precise:.bold} [{wide_bar:.blue/white.dim}]  {pos} frames ({fps:.bold})"
};

fn pretty_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(INDICATIF_PROGRESS_TEMPLATE)
        .unwrap()
        .with_key(
            "fps",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                if state.pos() == 0 || state.elapsed().as_secs_f32() < f32::EPSILON {
                    write!(w, "0 fps").unwrap();
                } else {
                    let fps = state.pos() as f32 / state.elapsed().as_secs_f32();
                    if fps < 1.0 {
                        write!(w, "{:.2} s/fr", 1.0 / fps).unwrap();
                    } else {
                        write!(w, "{:.2} fps", fps).unwrap();
                    }
                }
            },
        )
        .with_key(
            "fixed_eta",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                if state.pos() == 0 || state.elapsed().as_secs_f32() < f32::EPSILON {
                    write!(w, "unknown").unwrap();
                } else {
                    let spf = state.elapsed().as_secs_f32() / state.pos() as f32;
                    let remaining = state.len().unwrap_or(0) - state.pos();
                    write!(
                        w,
                        "{:#}",
                        HumanDuration(Duration::from_secs_f32(spf * remaining as f32))
                    )
                    .unwrap();
                }
            },
        )
        .with_key(
            "pos",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{}/{}", state.pos(), state.len().unwrap_or(0)).unwrap();
            },
        )
        .with_key(
            "percent",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{:>3.0}%", state.fraction() * 100_f32).unwrap();
            },
        )
        .progress_chars(PROGRESS_CHARS)
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template(INDICATIF_SPINNER_TEMPLATE)
        .unwrap()
        .with_key(
            "fps",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                if state.pos() == 0 || state.elapsed().as_secs_f32() < f32::EPSILON {
                    write!(w, "0 fps").unwrap();
                } else {
                    let fps = state.pos() as f32 / state.elapsed().as_secs_f32();
                    if fps < 1.0 {
                        write!(w, "{:.2} s/fr", 1.0 / fps).unwrap();
                    } else {
                        write!(w, "{:.2} fps", fps).unwrap();
                    }
                }
            },
        )
        .with_key(
            "pos",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{}", state.pos()).unwrap();
            },
        )
        .progress_chars(PROGRESS_CHARS)
}

fn calc_score<S: Pixel, D: Pixel>(
    mtx: &Mutex<(usize, (FfmpegDecoder, FfmpegDecoder))>,
    src_yuvcfg: &YuvConfig,
    dst_yuvcfg: &YuvConfig,
) -> Option<(usize, f64)> {
    let (frame_idx, (src_frame, dst_frame)) = {
        let mut guard = mtx.lock().unwrap();
        let curr_frame = guard.0;

        let src_frame = guard.1 .0.read_video_frame::<S>();
        let dst_frame = guard.1 .1.read_video_frame::<D>();

        if let (Some(sf), Some(df)) = (src_frame, dst_frame) {
            guard.0 += 1;
            (curr_frame, (sf, df))
        } else {
            return None;
        }
    };

    let src_yuv = Yuv::new(src_frame, *src_yuvcfg).unwrap();
    let dst_yuv = Yuv::new(dst_frame, *dst_yuvcfg).unwrap();

    Some((
        frame_idx,
        compute_frame_ssimulacra2(src_yuv, dst_yuv).expect("Failed to calculate ssimulacra2"),
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn compare_videos(
    source: &Path,
    distorted: &Path,
    frame_threads: usize,
    graph: bool,
    verbose: bool,
    mut src_matrix: MatrixCoefficients,
    mut src_transfer: TransferCharacteristic,
    mut src_primaries: ColorPrimaries,
    src_full_range: bool,
    mut dst_matrix: MatrixCoefficients,
    mut dst_transfer: TransferCharacteristic,
    mut dst_primaries: ColorPrimaries,
    dst_full_range: bool,
) {
    let frame_count = get_frame_count(source).ok();
    let source = FfmpegDecoder::new(source).unwrap();
    let distorted = FfmpegDecoder::new(distorted).unwrap();

    if src_matrix == MatrixCoefficients::Unspecified {
        src_matrix = guess_matrix_coefficients(
            source.get_video_details().width,
            source.get_video_details().height,
        );
    }
    if dst_matrix == MatrixCoefficients::Unspecified {
        dst_matrix = guess_matrix_coefficients(
            distorted.get_video_details().width,
            distorted.get_video_details().height,
        );
    }
    if src_transfer == TransferCharacteristic::Unspecified {
        src_transfer = TransferCharacteristic::BT1886;
    }
    if dst_transfer == TransferCharacteristic::Unspecified {
        dst_transfer = TransferCharacteristic::BT1886;
    }
    if src_primaries == ColorPrimaries::Unspecified {
        src_primaries = guess_color_primaries(
            src_matrix,
            source.get_video_details().width,
            source.get_video_details().height,
        );
    }
    if dst_primaries == ColorPrimaries::Unspecified {
        dst_primaries = guess_color_primaries(
            dst_matrix,
            distorted.get_video_details().width,
            distorted.get_video_details().height,
        );
    }

    let src_dec = source
        .get_video_details()
        .chroma_sampling
        .get_decimation()
        .unwrap_or((0, 0));
    let src_config = YuvConfig {
        bit_depth: source.get_bit_depth() as u8,
        subsampling_x: src_dec.0 as u8,
        subsampling_y: src_dec.1 as u8,
        full_range: src_full_range,
        matrix_coefficients: src_matrix,
        transfer_characteristics: src_transfer,
        color_primaries: src_primaries,
    };
    let dst_dec = distorted
        .get_video_details()
        .chroma_sampling
        .get_decimation()
        .unwrap_or((0, 0));
    let dst_config = YuvConfig {
        bit_depth: distorted.get_bit_depth() as u8,
        subsampling_x: dst_dec.0 as u8,
        subsampling_y: dst_dec.1 as u8,
        full_range: dst_full_range,
        matrix_coefficients: dst_matrix,
        transfer_characteristics: dst_transfer,
        color_primaries: dst_primaries,
    };

    let (result_tx, result_rx) = mpsc::channel();
    let src_bd = source.get_bit_depth();
    let dst_bd = distorted.get_bit_depth();

    let current_frame = 0usize;
    let decoders = Arc::new(Mutex::new((current_frame, (source, distorted))));
    for _ in 0..frame_threads {
        let decoders = Arc::clone(&decoders);
        let result_tx = result_tx.clone();

        std::thread::spawn(move || {
            loop {
                let score = match (src_bd, dst_bd) {
                    (8, 8) => calc_score::<u8, u8>(&decoders, &src_config, &dst_config),
                    (8, _) => calc_score::<u8, u16>(&decoders, &src_config, &dst_config),
                    (_, 8) => calc_score::<u16, u8>(&decoders, &src_config, &dst_config),
                    (_, _) => calc_score::<u16, u16>(&decoders, &src_config, &dst_config),
                };

                if let Some(result) = score {
                    result_tx.send(result).unwrap();
                } else {
                    // no score = no more frames to read
                    break;
                }
            }
        });
    }

    // Needs to be dropped or the main thread never stops waiting for scores
    drop(result_tx);

    let progress = if stderr().is_tty() {
        let pb = frame_count.map_or_else(
            || ProgressBar::new(0).with_style(spinner_style()),
            |frame_count| ProgressBar::new(frame_count as u64).with_style(pretty_progress_style()),
        );
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.reset();
        pb.reset_eta();
        pb.reset_elapsed();
        pb.set_position(0);
        pb
    } else {
        ProgressBar::hidden()
    };

    let mut results = BTreeMap::new();
    for score in result_rx {
        if verbose {
            println!("Frame {}: {:.8}", score.0, score.1);
        }

        results.insert(score.0, score.1);
        progress.inc(1);
    }

    progress.finish();

    let results: Vec<f64> = results.into_values().collect();
    let frames = results.len();
    let mut data = Data::new(results.clone());
    println!("Video Score for {} frames", frames);
    println!("Mean: {:.8}", data.mean().unwrap());
    println!("Median: {:.8}", data.median());
    println!("Std Dev: {:.8}", data.std_dev().unwrap());
    println!("5th Percentile: {:.8}", data.percentile(5));
    println!("95th Percentile: {:.8}", data.percentile(95));

    if graph {
        use plotters::prelude::*;

        let out_path = PathBuf::from(format!(
            "ssimulacra2-video-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ));
        let width = 1500u32;
        let height = 1000u32;
        let mut image_buffer = vec![0; (width * height * 3) as usize].into_boxed_slice();

        {
            let root =
                BitMapBackend::with_buffer(&mut image_buffer, (width, height)).into_drawing_area();
            root.fill(&BLACK).unwrap();
            let mut chart = ChartBuilder::on(&root)
                .set_label_area_size(LabelAreaPosition::Left, 60)
                .set_label_area_size(LabelAreaPosition::Bottom, 60)
                .caption("SSIMULACRA2", ("sans-serif", 50.0))
                .build_cartesian_2d(0..frames, 0f32..100f32)
                .unwrap();
            chart
                .configure_mesh()
                .disable_x_mesh()
                .bold_line_style(WHITE.mix(0.3))
                .y_desc("Score")
                .y_label_style(("sans-serif", 16, &WHITE))
                .x_desc("Frame")
                .x_label_style(("sans-serif", 16, &WHITE))
                .axis_desc_style(("sans-serif", 18, &WHITE))
                .draw()
                .unwrap();
            chart
                .draw_series(
                    AreaSeries::new(
                        results.into_iter().enumerate().map(|(i, v)| (i, v as f32)),
                        0.0,
                        CYAN.mix(0.5),
                    )
                    .border_style(CYAN.filled()),
                )
                .unwrap();
            root.present().expect("Unable to generate image");
        }

        image::save_buffer(&out_path, &image_buffer, width, height, ColorType::Rgb8)
            .expect("Unable to save graph image");

        println!();
        println!("Graph written to {}", out_path.to_string_lossy());
    }
}

pub fn parse_matrix(input: &str) -> MatrixCoefficients {
    if let Ok(intval) = input.parse::<u8>() {
        if intval <= MatrixCoefficients::ICtCp as u8 {
            return MatrixCoefficients::from_u8(intval).expect("Invalid matrix coefficient value");
        }
    }

    match input.to_ascii_lowercase().as_str() {
        "identity" | "rgb" | "srgb" | "smpte428" | "xyz" => MatrixCoefficients::Identity,
        "709" | "bt709" => MatrixCoefficients::BT709,
        "unspecified" => MatrixCoefficients::Unspecified,
        "bt470m" | "470m" => MatrixCoefficients::BT470M,
        "bt470bg" | "470bg" | "601-625" | "bt601-625" | "pal" => MatrixCoefficients::BT470BG,
        "smpte170m" | "170m" | "601-525" | "bt601-525" | "bt601" | "601" | "ntsc" => {
            MatrixCoefficients::ST170M
        }
        "240m" | "smpte240m" => MatrixCoefficients::ST240M,
        "ycgco" => MatrixCoefficients::YCgCo,
        "2020" | "2020ncl" | "2020-ncl" | "bt2020" | "bt2020ncl" | "bt2020-ncl" => {
            MatrixCoefficients::BT2020NonConstantLuminance
        }
        "2020cl" | "2020-cl" | "bt2020cl" | "bt2020-cl" => {
            MatrixCoefficients::BT2020ConstantLuminance
        }
        "2085" | "smpte2085" => MatrixCoefficients::ST2085,
        "cd-ncl" => MatrixCoefficients::ChromaticityDerivedNonConstantLuminance,
        "cd-cl" => MatrixCoefficients::ChromaticityDerivedConstantLuminance,
        "2100" | "bt2100" | "ictcp" => MatrixCoefficients::ICtCp,
        _ => panic!("Unrecognized matrix coefficient string"),
    }
}

pub fn parse_transfer(input: &str) -> TransferCharacteristic {
    if let Ok(intval) = input.parse::<u8>() {
        if intval <= TransferCharacteristic::HybridLogGamma as u8 {
            return TransferCharacteristic::from_u8(intval)
                .expect("Invalid transfer characteristics value");
        }
    }

    match input.to_ascii_lowercase().as_str() {
        "709" | "bt709" | "1886" | "bt1886" | "1361" | "bt1361" => TransferCharacteristic::BT1886,
        "unspecified" => TransferCharacteristic::Unspecified,
        "470m" | "bt470m" | "pal" => TransferCharacteristic::BT470M,
        "470bg" | "bt470bg" => TransferCharacteristic::BT470BG,
        "601" | "bt601" | "ntsc" | "smpte170m" | "170m" | "1358" | "bt1358" | "1700" | "bt1700" => {
            TransferCharacteristic::ST170M
        }
        "240m" | "smpte240m" => TransferCharacteristic::ST240M,
        "linear" => TransferCharacteristic::Linear,
        "log100" => TransferCharacteristic::Logarithmic100,
        "log316" => TransferCharacteristic::Logarithmic316,
        "xvycc" => TransferCharacteristic::XVYCC,
        "1361e" | "bt1361e" => TransferCharacteristic::BT1361E,
        "srgb" => TransferCharacteristic::SRGB,
        "2020" | "bt2020" | "2020-10" | "bt2020-10" => TransferCharacteristic::BT2020Ten,
        "2020-12" | "bt2020-12" => TransferCharacteristic::BT2020Twelve,
        "pq" | "2084" | "smpte2084" | "2100" | "bt2100" => {
            TransferCharacteristic::PerceptualQuantizer
        }
        "428" | "smpte428" => TransferCharacteristic::ST428,
        "hlg" | "b67" | "arib-b67" => TransferCharacteristic::HybridLogGamma,
        _ => panic!("Unrecognized transfer characteristics string"),
    }
}

pub fn parse_primaries(input: &str) -> ColorPrimaries {
    if let Ok(intval) = input.parse::<u8>() {
        if intval <= ColorPrimaries::Tech3213 as u8 {
            return ColorPrimaries::from_u8(intval).expect("Invalid color primaries value");
        }
    }

    match input.to_ascii_lowercase().as_str() {
        "709" | "bt709" | "1361" | "bt1361" | "srgb" => ColorPrimaries::BT709,
        "unspecified" => ColorPrimaries::Unspecified,
        "470m" | "bt470m" => ColorPrimaries::BT470M,
        "470bg" | "bt470bg" | "601-625" | "bt601-625" | "pal" => ColorPrimaries::BT470BG,
        "smpte170m" | "170m" | "601-525" | "bt601-525" | "bt601" | "601" | "ntsc" => {
            ColorPrimaries::ST170M
        }
        "240m" | "smpte240m" => ColorPrimaries::ST240M,
        "film" | "c" => ColorPrimaries::Film,
        "2020" | "bt2020" | "2100" | "bt2100" => ColorPrimaries::BT2020,
        "428" | "smpte428" | "xyz" => ColorPrimaries::ST428,
        "p3" | "p3dci" | "p3-dci" | "431" | "smpte431" => ColorPrimaries::P3DCI,
        "p3display" | "p3-display" | "432" | "smpte432" => ColorPrimaries::P3Display,
        "3213" | "tech3213" => ColorPrimaries::Tech3213,
        _ => panic!("Unrecognized color primaries string"),
    }
}

pub const fn guess_matrix_coefficients(width: usize, height: usize) -> MatrixCoefficients {
    if width >= 1280 || height > 576 {
        MatrixCoefficients::BT709
    } else if height == 576 {
        MatrixCoefficients::BT470BG
    } else {
        MatrixCoefficients::ST170M
    }
}

// Heuristic taken from mpv
pub fn guess_color_primaries(
    matrix: MatrixCoefficients,
    width: usize,
    height: usize,
) -> ColorPrimaries {
    if matrix == MatrixCoefficients::BT2020NonConstantLuminance
        || matrix == MatrixCoefficients::BT2020ConstantLuminance
    {
        ColorPrimaries::BT2020
    } else if matrix == MatrixCoefficients::BT709 || width >= 1280 || height > 576 {
        ColorPrimaries::BT709
    } else if height == 576 {
        ColorPrimaries::BT470BG
    } else if height == 480 || height == 488 {
        ColorPrimaries::ST170M
    } else {
        ColorPrimaries::BT709
    }
}

fn get_frame_count(video: &Path) -> Result<usize> {
    // Would it be better to use the ffmpeg API for this? Yes.
    // But it would also be an outrageous pain in the rear,
    // when I can use the command line by copy and pasting
    // one command from StackOverflow.
    let result = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-count_packets")
        .arg("-show_entries")
        .arg("stream=nb_read_packets")
        .arg("-of")
        .arg("csv=p=0")
        .arg(video)
        .output()?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    Ok(stdout.trim().parse()?)
}
