use std::cmp::min;
use std::collections::BTreeMap;
use std::io::stderr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use av_metrics_decoders::{y4m::new_decoder_from_stdin, Decoder, VapoursynthDecoder};
use crossterm::tty::IsTty;
use image::ColorType;
use indicatif::{HumanDuration, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use num_traits::FromPrimitive;
use ssimulacra2::{
    compute_frame_ssimulacra2, ColorPrimaries, MatrixCoefficients, Pixel, TransferCharacteristic,
    Yuv, YuvConfig,
};
use statrs::statistics::{Data, Distribution, Median, OrderStatistics};

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
    "{elapsed_precise:.bold} {pos} ({fps:.bold}{msg})"
} else {
    "{spinner:.green.bold} {elapsed_precise:.bold} {pos} ({fps:.bold}{msg})"
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

fn pretty_spinner_style() -> ProgressStyle {
    ProgressStyle::default_bar()
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

type VideoCompareMutex<E, F> = Arc<Mutex<VideoCompare<E, F>>>;

struct VideoCompare<E: Decoder, F: Decoder> {
    current_frame: usize,
    next_frame: usize,
    source: E,
    distorted: F,
}

fn calc_score<S: Pixel, D: Pixel, E: Decoder, F: Decoder>(
    mtx: &VideoCompareMutex<E, F>,
    src_yuvcfg: &YuvConfig,
    dst_yuvcfg: &YuvConfig,
    inc: usize,
    end_frame: Option<usize>,
    verbose: bool,
) -> Option<(usize, f64)> {
    let (frame_idx, (src_frame, dst_frame)) = {
        let mut guard = mtx.lock().unwrap();
        let mut curr_frame = guard.current_frame;
        // We passed this as start frame.
        // However, we are going to use it to store the next frame we should compute.
        let mut next_frame = guard.next_frame;

        //println!("{curr_frame} < {next_frame}");

        while curr_frame < next_frame {
            let _src_frame = guard.source.read_video_frame::<S>();
            let _dst_frame = guard.distorted.read_video_frame::<D>();
            if _src_frame.is_none() || _dst_frame.is_none() {
                break;
            }
            if verbose {
                println!("Frame {}: skip", curr_frame);
            }
            curr_frame += 1;
        }

        next_frame = curr_frame + inc;

        if let Some(end_frame) = end_frame {
            if next_frame > end_frame {
                return None;
            }
        }

        let src_frame = guard.source.read_video_frame::<S>();
        let dst_frame = guard.distorted.read_video_frame::<D>();

        guard.current_frame = curr_frame + 1;
        guard.next_frame = next_frame;

        //println!("current: {}, next: {}", curr_frame, next_frame);

        if let (Some(sf), Some(df)) = (src_frame, dst_frame) {
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
    source: &str,
    distorted: &str,
    frame_threads: usize,
    start_frame: Option<usize>,
    frames_to_compare: Option<usize>,
    inc: usize,
    graph: bool,
    verbose: bool,
    src_matrix: MatrixCoefficients,
    src_transfer: TransferCharacteristic,
    src_primaries: ColorPrimaries,
    src_full_range: bool,
    dst_matrix: MatrixCoefficients,
    dst_transfer: TransferCharacteristic,
    dst_primaries: ColorPrimaries,
    dst_full_range: bool,
) {
    if source == "-" || source == "/dev/stdin" {
        assert!(
            !(distorted == "-" || distorted == "/dev/stdin"),
            "Source and distorted inputs cannot both be from piped input"
        );
        let distorted = if Path::new(distorted)
            .extension()
            .map(|ext| ext.to_ascii_lowercase().to_string_lossy() == "vpy")
            .unwrap_or(false)
        {
            VapoursynthDecoder::new_from_script(Path::new(distorted)).unwrap()
        } else {
            VapoursynthDecoder::new_from_video(Path::new(distorted)).unwrap()
        };
        let distorted_frame_count = distorted.get_frame_count().ok();
        return compare_videos_inner(
            new_decoder_from_stdin().unwrap(),
            distorted,
            None,
            distorted_frame_count,
            frame_threads,
            start_frame,
            frames_to_compare,
            inc,
            graph,
            verbose,
            src_matrix,
            src_transfer,
            src_primaries,
            src_full_range,
            dst_matrix,
            dst_transfer,
            dst_primaries,
            dst_full_range,
        );
    }

    if distorted == "-" || distorted == "/dev/stdin" {
        let source = if Path::new(source)
            .extension()
            .map(|ext| ext.to_ascii_lowercase().to_string_lossy() == "vpy")
            .unwrap_or(false)
        {
            VapoursynthDecoder::new_from_script(Path::new(source)).unwrap()
        } else {
            VapoursynthDecoder::new_from_video(Path::new(source)).unwrap()
        };
        let source_frame_count = source.get_frame_count().ok();
        return compare_videos_inner(
            source,
            new_decoder_from_stdin().unwrap(),
            source_frame_count,
            None,
            frame_threads,
            start_frame,
            frames_to_compare,
            inc,
            graph,
            verbose,
            src_matrix,
            src_transfer,
            src_primaries,
            src_full_range,
            dst_matrix,
            dst_transfer,
            dst_primaries,
            dst_full_range,
        );
    }

    let source = if Path::new(source)
        .extension()
        .map(|ext| ext.to_ascii_lowercase().to_string_lossy() == "vpy")
        .unwrap_or(false)
    {
        VapoursynthDecoder::new_from_script(Path::new(source)).unwrap()
    } else {
        VapoursynthDecoder::new_from_video(Path::new(source)).unwrap()
    };
    let distorted = if Path::new(distorted)
        .extension()
        .map(|ext| ext.to_ascii_lowercase().to_string_lossy() == "vpy")
        .unwrap_or(false)
    {
        VapoursynthDecoder::new_from_script(Path::new(distorted)).unwrap()
    } else {
        VapoursynthDecoder::new_from_video(Path::new(distorted)).unwrap()
    };
    let source_frame_count = source.get_frame_count().ok();
    let distorted_frame_count = distorted.get_frame_count().ok();
    compare_videos_inner(
        source,
        distorted,
        source_frame_count,
        distorted_frame_count,
        frame_threads,
        start_frame,
        frames_to_compare,
        inc,
        graph,
        verbose,
        src_matrix,
        src_transfer,
        src_primaries,
        src_full_range,
        dst_matrix,
        dst_transfer,
        dst_primaries,
        dst_full_range,
    )
}

#[allow(clippy::too_many_arguments)]
fn compare_videos_inner<D: Decoder + 'static, E: Decoder + 'static>(
    source: D,
    distorted: E,
    source_frame_count: Option<usize>,
    distorted_frame_count: Option<usize>,
    frame_threads: usize,
    start_frame: Option<usize>,
    frames_to_compare: Option<usize>,
    inc: usize,
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
    if let Some(source_frame_count) = source_frame_count {
        if let Some(distorted_frame_count) = distorted_frame_count {
            if source_frame_count != distorted_frame_count {
                eprintln!("WARNING: Frame count mismatch detected, scores may be inaccurate");
            }
        }
    }


    if start_frame.is_some() {
        assert!(
            source_frame_count.is_some() || distorted_frame_count.is_some(),
            "--start-frame was used, but we could not get source or distorted frame count"
        )
    }


    let source_info = source.get_video_details();
    let distorted_info = distorted.get_video_details();
    if src_matrix == MatrixCoefficients::Unspecified {
        src_matrix = guess_matrix_coefficients(source_info.width, source_info.height);
    }
    if dst_matrix == MatrixCoefficients::Unspecified {
        dst_matrix = guess_matrix_coefficients(distorted_info.width, distorted_info.height);
    }
    if src_transfer == TransferCharacteristic::Unspecified {
        src_transfer = TransferCharacteristic::BT1886;
    }
    if dst_transfer == TransferCharacteristic::Unspecified {
        dst_transfer = TransferCharacteristic::BT1886;
    }
    if src_primaries == ColorPrimaries::Unspecified {
        src_primaries = guess_color_primaries(src_matrix, source_info.width, source_info.height);
    }
    if dst_primaries == ColorPrimaries::Unspecified {
        dst_primaries =
            guess_color_primaries(dst_matrix, distorted_info.width, distorted_info.height);
    }

    let src_ss = source_info
        .chroma_sampling
        .get_decimation()
        .unwrap_or((0, 0));
    let dist_ss = distorted_info
        .chroma_sampling
        .get_decimation()
        .unwrap_or((0, 0));
    let src_config = YuvConfig {
        bit_depth: source_info.bit_depth as u8,
        subsampling_x: src_ss.0 as u8,
        subsampling_y: src_ss.1 as u8,
        full_range: src_full_range,
        matrix_coefficients: src_matrix,
        transfer_characteristics: src_transfer,
        color_primaries: src_primaries,
    };
    let dst_config = YuvConfig {
        bit_depth: distorted_info.bit_depth as u8,
        subsampling_x: dist_ss.0 as u8,
        subsampling_y: dist_ss.1 as u8,
        full_range: dst_full_range,
        matrix_coefficients: dst_matrix,
        transfer_characteristics: dst_transfer,
        color_primaries: dst_primaries,
    };

    let (result_tx, result_rx) = mpsc::channel();
    let src_bd = src_config.bit_depth;
    let dst_bd = dst_config.bit_depth;

    let current_frame = 0usize;
    let start_frame = start_frame.unwrap_or(0);
    let end_frame = frames_to_compare
        .map(|frames_to_compare| start_frame + (frames_to_compare * inc));

    let video_compare = Arc::new(
        Mutex::new(
            VideoCompare {
                current_frame,
                next_frame: start_frame,
                source,
                distorted,
            }
        )
    );

    for _ in 0..frame_threads {
        let video_compare = Arc::clone(&video_compare);
        let result_tx = result_tx.clone();

        std::thread::spawn(move || {
            loop {
                let score = match (src_bd, dst_bd) {
                    (8, 8) => calc_score::<u8, u8, _, _>(
                        &video_compare,
                        &src_config,
                        &dst_config,
                        inc,
                        end_frame,
                        verbose,
                    ),
                    (8, _) => calc_score::<u8, u16, _, _>(
                        &video_compare,
                        &src_config,
                        &dst_config,
                        inc,
                        end_frame,
                        verbose,
                    ),
                    (_, 8) => calc_score::<u16, u8, _, _>(
                        &video_compare,
                        &src_config,
                        &dst_config,
                        inc,
                        end_frame,
                        verbose,
                    ),
                    (_, _) => calc_score::<u16, u16, _, _>(
                        &video_compare,
                        &src_config,
                        &dst_config,
                        inc,
                        end_frame,
                        verbose,
                    ),
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

    let progress = if stderr().is_tty() && !verbose {
        let frame_count = source_frame_count.or(distorted_frame_count);
        let pb = if let Some(frame_count) = frame_count {
            let fc = frames_to_compare.unwrap_or(frame_count - start_frame)
                .min((frame_count as f64 / inc as f64).ceil() as usize);

            ProgressBar::new(fc as u64)
                .with_style(pretty_progress_style())
                .with_message(", avg: N/A")
        } else {
            ProgressBar::new_spinner().with_style(pretty_spinner_style())
        };
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
    let mut avg = 0f64;
    for score in result_rx {
        if verbose {
            println!("Frame {}: {:.8}", score.0, score.1);
        }

        results.insert(score.0, score.1);
        avg = avg + (score.1 - avg) / (min(results.len(), 10) as f64);
        progress.set_message(format!(", avg: {:.1$}", avg, 2));
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
