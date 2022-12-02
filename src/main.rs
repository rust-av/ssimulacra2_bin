#[cfg(feature = "video")]
mod vapoursynth;
#[cfg(feature = "video")]
mod video;

#[cfg(feature = "video")]
use self::video::*;
use clap::{Parser, Subcommand};
#[cfg(feature = "video")]
use ssimulacra2::MatrixCoefficients;
use ssimulacra2::{compute_frame_ssimulacra2, ColorPrimaries, Rgb, TransferCharacteristic, Xyb};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compare two still images. Resolutions must be identical.
    Image {
        /// Source image
        #[arg(help = "Original unmodified image", value_hint = clap::ValueHint::FilePath)]
        source: PathBuf,

        /// Distorted image
        #[arg(help = "Distorted image", value_hint = clap::ValueHint::FilePath)]
        distorted: PathBuf,
    },
    /// Compare two videos. Resolutions and frame counts must be identical.
    #[cfg(feature = "video")]
    Video {
        /// Source video
        #[arg(help = "Original unmodified video", value_hint = clap::ValueHint::FilePath)]
        source: PathBuf,

        /// Distorted video
        #[arg(help = "Distorted video", value_hint = clap::ValueHint::FilePath)]
        distorted: PathBuf,

        /// How many worker threads to use for decoding & calculating scores.
        /// Note: Memory usage increases linearly with the number of workers.
        #[arg(long, short)]
        frame_threads: Option<usize>,

        /// Whether to output a frame-by-frame graph of scores.
        #[arg(long, short)]
        graph: bool,

        /// Will output scores for every frame followed by the average at the end.
        #[arg(long, short)]
        verbose: bool,

        /// Source color matrix
        #[arg(long)]
        src_matrix: Option<String>,

        /// Source transfer characteristics
        #[arg(long)]
        src_transfer: Option<String>,

        /// Source color primaries
        #[arg(long)]
        src_primaries: Option<String>,

        /// The source is using full-range data
        #[arg(long)]
        src_full_range: bool,

        /// Distorted color matrix
        #[arg(long)]
        dst_matrix: Option<String>,

        /// Distorted transfer characteristics
        #[arg(long)]
        dst_transfer: Option<String>,

        /// Distorted color primaries
        #[arg(long)]
        dst_primaries: Option<String>,

        /// The distorted video is using full-range data
        #[arg(long)]
        dst_full_range: bool,
    },
}

fn main() {
    match Cli::parse().command {
        Commands::Image { source, distorted } => compare_images(&source, &distorted),
        #[cfg(feature = "video")]
        Commands::Video {
            source,
            distorted,
            frame_threads,
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
        } => {
            let frame_threads = frame_threads.unwrap_or(1).max(1);
            let src_matrix = src_matrix
                .map(|i| parse_matrix(&i))
                .unwrap_or(MatrixCoefficients::Unspecified);
            let src_transfer = src_transfer
                .map(|i| parse_transfer(&i))
                .unwrap_or(TransferCharacteristic::Unspecified);
            let src_primaries = src_primaries
                .map(|i| parse_primaries(&i))
                .unwrap_or(ColorPrimaries::Unspecified);
            let dst_matrix = dst_matrix
                .map(|i| parse_matrix(&i))
                .unwrap_or(MatrixCoefficients::Unspecified);
            let dst_transfer = dst_transfer
                .map(|i| parse_transfer(&i))
                .unwrap_or(TransferCharacteristic::Unspecified);
            let dst_primaries = dst_primaries
                .map(|i| parse_primaries(&i))
                .unwrap_or(ColorPrimaries::Unspecified);
            compare_videos(
                &source,
                &distorted,
                frame_threads,
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
    }
}

fn compare_images(source: &Path, distorted: &Path) {
    // For now just assumes the input is sRGB. Trying to keep this as simple as possible for now.
    let source = image::open(source).expect("Failed to open source file");
    let distorted = image::open(distorted).expect("Failed to open distorted file");

    let source_data = source
        .to_rgb32f()
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect::<Vec<_>>();

    let source_data = Xyb::try_from(
        Rgb::new(
            source_data,
            source.width() as usize,
            source.height() as usize,
            TransferCharacteristic::SRGB,
            ColorPrimaries::BT709,
        )
        .expect("Failed to process source_data into RGB"),
    )
    .expect("Failed to process source_data into XYB");

    let distorted_data = distorted
        .to_rgb32f()
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect::<Vec<_>>();

    let distorted_data = Xyb::try_from(
        Rgb::new(
            distorted_data,
            distorted.width() as usize,
            distorted.height() as usize,
            TransferCharacteristic::SRGB,
            ColorPrimaries::BT709,
        )
        .expect("Failed to process distorted_data into RGB"),
    )
    .expect("Failed to process distorted_data into XYB");

    let result = compute_frame_ssimulacra2(source_data, distorted_data)
        .expect("Failed to calculate ssimulacra2");

    println!("Score: {result:.8}");
}
