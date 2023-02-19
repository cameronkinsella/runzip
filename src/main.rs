use clap::{arg, Parser};
use encoding_rs::{Encoding, UTF_8};
use std::path::{Path, PathBuf};
use std::{fs, io, process};
use zip::read::ZipFile;
use zip::ZipArchive;

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

    /// Codec to be used for filename encoding (default = UTF-8)
    #[arg(long, short = 'e')]
    encoding: Option<String>,

    /// Verbose output
    #[arg(long, short = 'v', default_value_t = false)]
    verbose: bool,
}

fn main() {
    let args: Args = Args::parse();

    // Open zip file
    let zip_path = std::path::Path::new(&args.file);
    let zip = match fs::File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Unable to open zip file: {e}");
            process::exit(1);
        }
    };

    // Parse zip file
    let mut archive = match ZipArchive::new(zip) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("Unable to parse zip file: {e}");
            process::exit(1);
        }
    };

    // Set output destination
    let mut destination = Path::new(zip_path.file_stem().unwrap()).to_path_buf();
    if let Some(out) = args.out {
        let output_meta = match fs::metadata(&out) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Invalid output directory: {e}");
                process::exit(1);
            }
        };
        if output_meta.is_file() {
            eprintln!("Error: output directory is not a directory");
            process::exit(1);
        }
        destination = out;
    }

    // Get encoding
    let mut encoding = UTF_8;
    if let Some(enc) = args.encoding {
        encoding = Encoding::for_label(enc.as_str().as_bytes()).unwrap_or_else(|| {
            eprintln!("Invalid encoding provided");
            process::exit(1);
        });
    }
    println!(
        "Archive: {:?}\nEncoding: {}",
        zip_path.file_name().unwrap(),
        encoding.name()
    );

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = match inflate(&mut file, &destination, encoding) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Unable to extract file {}: {e}", file.name());
                process::exit(1);
            }
        };

        if args.verbose {
            {
                let comment = file.comment();
                if !comment.is_empty() {
                    println!("\tFile {i} comment: {comment}");
                }
            }
            if outpath.is_dir() {
                println!("\tFile {i} extracted to \"{}\"", outpath.display());
            } else {
                println!(
                    "\tFile {i} extracted to \"{}\" ({} bytes)",
                    outpath.display(),
                    file.size()
                );
            }
        }
    }

    println!(
        "Extracted {} files to \"{}\"",
        archive.len(),
        fs::canonicalize(destination).unwrap().display()
    );
}

fn inflate(
    file: &mut ZipFile,
    destination: &PathBuf,
    encoding: &'static Encoding,
) -> Result<PathBuf, io::Error> {
    let (outpath, _enc, errors) = encoding.decode(file.name_raw());
    if errors {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to decode filename: {outpath}"),
        ));
    }
    let outpath = Path::new(&destination).join(outpath.as_ref());

    if (*file.name()).ends_with('/') {
        // Create directory
        fs::create_dir_all(&outpath)?;
    } else {
        // Create file
        if let Some(p) = outpath.parent() {
            if !p.exists() {
                fs::create_dir_all(p)?;
            }
        }
        let mut outfile = fs::File::create(&outpath)?;
        io::copy(file, &mut outfile)?;
    }

    // Get and Set permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Some(mode) = file.unix_mode() {
            fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(outpath)
}
