use clap::{arg, Parser};
use std::path::{Path, PathBuf};
use std::{fs, io};

/// Tool for extracting zip archives
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the zip archive
    #[arg(index = 1)]
    file: PathBuf,

    /// Output location.
    /// Extracts to a new folder in the current directory if none given.
    #[arg(long, short = 'o')]
    out: Option<PathBuf>,

    /// Password if the archive is encrypted
    #[arg(long, short = 'p')]
    password: Option<String>,

    /// Codec to be used for filename encoding (default: UTF-8)
    #[arg(long, short = 'e')]
    encoding: Option<String>,

    /// Make output less verbose
    #[arg(long, short = 's', default_value_t = false)]
    silent: bool,

    /// Only create a new directory if the archive contains files
    #[arg(long, default_value_t = false)]
    smart: bool,

    /// Overwrite folder names in smart mode
    #[arg(long, short = 'f', default_value_t = false)]
    force: bool,
}

fn main() -> Result<(), runzip::Error> {
    let args: Args = Args::parse();

    // Open archive file
    let archive_path = Path::new(&args.file);

    // Set output destination
    let mut destination = Path::new(archive_path.file_stem().unwrap()).to_path_buf();
    if args.smart {
        destination = PathBuf::from(archive_path.parent().unwrap().to_str().unwrap());
        destination.push(uuid::Uuid::new_v4().to_string())
    } else if let Some(out) = args.out {
        let output_meta = match fs::metadata(&out) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Invalid output directory");
                return Err(runzip::Error::Io(e));
            }
        };
        if output_meta.is_file() {
            return Err(runzip::Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Output directory is not a directory",
            )));
        }
        destination = out;
    }

    // Handle rar or zip archives
    let file_count = if archive_path.extension().and_then(std::ffi::OsStr::to_str) == Some("rar") {
        runzip::rar::unrar(
            archive_path,
            destination.as_path(),
            args.silent,
            args.password,
        )?
    } else {
        runzip::zip::unzip(
            archive_path,
            destination.as_path(),
            args.silent,
            args.password,
            args.encoding,
        )?
    };

    if args.smart {
        // Check if the directory only contains folders
        runzip::utils::process_directory(
            &mut destination,
            archive_path.file_stem().unwrap().to_str().unwrap(),
            args.force,
        )
        .expect("Unable to smart process output directory");
    }

    // TODO add byte units
    println!(
        "Extracted {} files to \"{}\"",
        file_count,
        fs::canonicalize(destination).unwrap().display()
    );
    Ok(())
}

// TODO: create Byte enum and impl display with 3 sig figs
//  0-999 Bytes, 1.00-999KB, etc.
