use clap::{Parser};
use std::{path::Path};
use std::sync::{Mutex, Arc};
use ssimulacra2::{compute_frame_ssimulacra2, ColorPrimaries, TransferCharacteristic, Xyb};
use std::fs;
use progress_bar::*;
use yuvxyb::Rgb;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Source image
    #[arg(help = "Original unmodified image", value_hint = clap::ValueHint::FilePath)]
    source: String,

    /// Distorted image
    #[arg(help = "Distorted image", value_hint = clap::ValueHint::FilePath)]
    distorted: String,

    /// Location to output a .csv file with the ssimumulacra2 values
    #[arg(help = "Output location. Requires --folders", value_hint = clap::ValueHint::FilePath, requires = "folders")]
    out: Option<String>,

    // TODO: Change help text to something more useful
    /// If input paths are folders, process all images in the folders
    #[arg(
        short,
        long,
        help = "If input paths are folders, process all images in the folders. This assumes the files are named the same in both folders."
    )]
    folders: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if !args.folders {
        let result = parse(args.source, args.distorted);
        println!("{result:.8}");
    } else {
        // args get's moved into handle_folder, so we need to clone `out`
        let out_clone = args.out.clone();

        let mut results = handle_folder(args).await;

        // Sort by frame number
        results.sort_by(|a, b| a.frame.cmp(&b.frame));

        // println!("{:#?}", results);

        // Print Mean, min, max
        println!("Min: {}", results.iter().map(|r| r.ssimulacra2).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap());
        println!("Max: {}", results.iter().map(|r| r.ssimulacra2).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap());
        println!("Mean: {}", results.iter().map(|r| r.ssimulacra2).sum::<f64>() / results.len() as f64);

        // Print CSV
        if let Some(out) = out_clone {
            let mut csv = String::new();
            csv.push_str("frame,ssimulacra2\n");
            for result in results {
                csv.push_str(&format!("{},{}\n", result.frame, result.ssimulacra2));
            }
            // check if `out` is a directory
            if Path::new(&out).is_dir() {
                let mut path = Path::new(&out).to_path_buf();
                path.push("ssimulacra2.csv");
                fs::write(path, csv).expect("Unable to write file");
            } else {
                fs::write(out, csv).expect("Unable to write file");
            }
        }
    }
}

fn parse(source_path: String, distorted_path: String) -> f64 {
    // For now just assumes the input is sRGB. Trying to keep this as simple as possible for now.
    let source = image::open(source_path).expect("Failed to open source file");
    let distorted = image::open(distorted_path).expect("Failed to open distorted file");

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

    // Compute and return the SSIMulacra2 value
    compute_frame_ssimulacra2(source_data, distorted_data).expect("Failed to calculate ssimulacra2")
}

async fn handle_folder(args: Args) -> Vec<FrameResult> {
    let files = fs::read_dir(args.source.clone()).unwrap();

        let results: Arc<Mutex<Vec<FrameResult>>> = Arc::new(Mutex::new(Vec::new()));

        // let mut count = 0;

        // TODO: This is a bit ugly, but it works. Reopen the folder and iterate over it again because count consumes the iterator.
        let len = fs::read_dir(args.source.clone()).unwrap().count();

        println!("Processing {} files", len);

        let mut handles = vec![];

        init_progress_bar(len);
        set_progress_bar_action("Processing", Color::Blue, Style::Bold);

        // TODO: Figure out how to multithread this? 
        for (count, path) in files.enumerate() {
            // count += 1;

            let arg_source = args.source.clone();
            let arg_distorted = args.distorted.clone();

            let results_clone = Arc::clone(&results);

            handles.push(tokio::spawn(async move {
            
            let src_path = Path::new(&arg_source);
            let dst_path = Path::new(&arg_distorted);

            let file_name = path.unwrap().file_name();

            let ssimulacra2_result = parse(
                src_path.join(file_name.clone()).to_str().unwrap().to_owned(),
                dst_path.join(file_name).to_str().unwrap().to_owned(),
            );

            results_clone.lock().unwrap().push(FrameResult{
                frame: count.try_into().unwrap(),
                ssimulacra2: ssimulacra2_result,
            });

            // println!("Frame {}/{} complete!", count, len);
            inc_progress_bar();

            }));

        }

        futures::future::join_all(handles).await;

        finalize_progress_bar();

        let x = results.lock().unwrap().to_vec(); x
}

// struct to hold frame number and ssimulacra2 value
#[derive(Debug, Clone)]
struct FrameResult {
    frame: u32,
    ssimulacra2: f64,
}