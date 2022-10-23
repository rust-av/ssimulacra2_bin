use clap::Parser;
use ssimulacra2::{compute_frame_ssimulacra2, ColorPrimaries, TransferCharacteristic, Xyb};
use yuvxyb::Rgb;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Source image
    #[arg(help = "Origional unmodified image", value_hint = clap::ValueHint::FilePath)]
    source: String,

    /// Distorted image
    #[arg(help = "Distorted image", value_hint = clap::ValueHint::FilePath)]
    distorted: String,
}

fn main() {
    let args = Args::parse();

    // For now just assumes the input is sRGB. Trying to keep this as simple as possible for now.
    let source = image::open(args.source).expect("Failed to open source file");
    let distorted = image::open(args.distorted).expect("Failed to open distorted file");

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

    println!("{result:.8}");
}
