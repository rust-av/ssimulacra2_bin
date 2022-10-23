use std::{env::args, process::exit};

use ssimulacra2::{compute_frame_ssimulacra2, ColorPrimaries, TransferCharacteristic, Xyb};
use yuvxyb::Rgb;

fn main() {
    let args = args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: ssimulacra2_rs source.png distorted.png");
        exit(0);
    }

    // For now just assumes the input is sRGB. Trying to keep this as simple as possible for now.
    let source = image::open(&args[0]).expect("Failed to open source file");
    let distorted = image::open(&args[1]).expect("Failed to open distorted file");
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
        .unwrap(),
    )
    .unwrap();
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
        .unwrap(),
    )
    .unwrap();
    let result = compute_frame_ssimulacra2(source_data, distorted_data).unwrap();
    eprintln!("{:.8}", result);
}
