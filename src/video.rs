use std::io::stderr;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use av_metrics_decoders::{Decoder, FfmpegDecoder};
use indicatif::{ProgressBar, ProgressStyle};
use num_traits::FromPrimitive;
use ssimulacra2::{
    compute_frame_ssimulacra2, ColorPrimaries, MatrixCoefficients, Rgb, TransferCharacteristic,
    Xyb, Yuv, YuvConfig,
};
use statrs::statistics::{Data, Distribution, Median, OrderStatistics};

#[allow(clippy::too_many_arguments)]
pub fn compare_videos(
    source: &Path,
    distorted: &Path,
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
    let mut progress = if termion::is_tty(&stderr()) {
        ProgressBar::new_spinner().with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise:.blue}] [{per_sec:.green}] Frame {pos}",
            )
            .unwrap(),
        )
    } else {
        ProgressBar::hidden()
    };

    let mut source = FfmpegDecoder::new(source).unwrap();
    let mut distorted = FfmpegDecoder::new(distorted).unwrap();

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

    let mut results = Vec::new();
    let mut frame = 0usize;
    loop {
        let (src_rgb, dst_rgb) = match (source.get_bit_depth(), distorted.get_bit_depth()) {
            (8, 8) => {
                let src_frame = source.read_video_frame::<u8>();
                let dst_frame = distorted.read_video_frame::<u8>();
                if let (Some(src_frame), Some(dst_frame)) = (src_frame, dst_frame) {
                    let src = Yuv::new(src_frame, src_config).unwrap();
                    let dst = Yuv::new(dst_frame, dst_config).unwrap();
                    (Rgb::try_from(src).unwrap(), Rgb::try_from(dst).unwrap())
                } else {
                    break;
                }
            }
            (8, _) => {
                let src_frame = source.read_video_frame::<u8>();
                let dst_frame = distorted.read_video_frame::<u16>();
                if let (Some(src_frame), Some(dst_frame)) = (src_frame, dst_frame) {
                    let src = Yuv::new(src_frame, src_config).unwrap();
                    let dst = Yuv::new(dst_frame, dst_config).unwrap();
                    (Rgb::try_from(src).unwrap(), Rgb::try_from(dst).unwrap())
                } else {
                    break;
                }
            }
            (_, 8) => {
                let src_frame = source.read_video_frame::<u16>();
                let dst_frame = distorted.read_video_frame::<u8>();
                if let (Some(src_frame), Some(dst_frame)) = (src_frame, dst_frame) {
                    let src = Yuv::new(src_frame, src_config).unwrap();
                    let dst = Yuv::new(dst_frame, dst_config).unwrap();
                    (Rgb::try_from(src).unwrap(), Rgb::try_from(dst).unwrap())
                } else {
                    break;
                }
            }
            (_, _) => {
                let src_frame = source.read_video_frame::<u16>();
                let dst_frame = distorted.read_video_frame::<u16>();
                if let (Some(src_frame), Some(dst_frame)) = (src_frame, dst_frame) {
                    let src = Yuv::new(src_frame, src_config).unwrap();
                    let dst = Yuv::new(dst_frame, dst_config).unwrap();
                    (Rgb::try_from(src).unwrap(), Rgb::try_from(dst).unwrap())
                } else {
                    break;
                }
            }
        };
        let src_xyb = Xyb::try_from(src_rgb).unwrap();
        let dst_xyb = Xyb::try_from(dst_rgb).unwrap();
        let result =
            compute_frame_ssimulacra2(src_xyb, dst_xyb).expect("Failed to calculate ssimulacra2");
        if verbose {
            println!("Frame {frame}: {result:.8}");
        }
        results.push(result);
        frame += 1;
        progress.inc(1);
    }

    progress.finish();
    let mut data = Data::new(results.clone());
    println!("Video Score for {} frames", frame);
    println!("Mean: {:.8}", data.mean().unwrap());
    println!("Median: {:.8}", data.median());
    println!("Std Dev: {:.8}", data.std_dev().unwrap());
    println!("5th Percentile: {:.8}", data.percentile(5));
    println!("95th Percentile: {:.8}", data.percentile(95));

    if graph {
        use plotters::prelude::*;

        let out_path = PathBuf::from(format!(
            "ssimulacra2-video-{}.svg",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ));

        let root = SVGBackend::new(&out_path, (1500, 1000)).into_drawing_area();
        root.fill(&BLACK).unwrap();
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .margin(5)
            .caption("SSIMULACRA2", ("sans-serif", 50.0))
            .build_cartesian_2d(0..frame, 0f32..100f32)
            .unwrap();
        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(&WHITE.mix(0.3))
            .y_desc("Score")
            .y_label_style(("sans-serif", 16, &WHITE))
            .x_desc("Frame")
            .x_label_style(("sans-serif", 16, &WHITE))
            .axis_desc_style(("sans-serif", 18, &WHITE))
            .draw()
            .unwrap();
        chart
            .draw_series(
                Histogram::vertical(&chart)
                    .style(CYAN.mix(0.5).filled())
                    .data(results.into_iter().enumerate().map(|(i, v)| (i, v as f32))),
            )
            .unwrap();
        root.present().expect("Unable to write result to file");

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
