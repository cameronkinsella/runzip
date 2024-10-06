use clap::{arg, Parser};
use encoding_rs::{Encoding, UTF_8};
use std::iter::successors;
use std::path::{Path, PathBuf};
use std::{fs, io};
use zip::read::ZipFile;
use zip::result::{InvalidPassword, ZipError};
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

#[derive(Debug)]
enum Error {
    Io(io::Error),
    Zip(ZipError),
    InvalidPassword,
    EncodingError,
}

impl From<ZipError> for Error {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<InvalidPassword> for Error {
    fn from(_: InvalidPassword) -> Self {
        Self::InvalidPassword
    }
}

fn main() -> Result<(), Error> {
    let args: Args = Args::parse();

    // Open zip file
    let zip_path = std::path::Path::new(&args.file);
    let zip = match fs::File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Unable to open zip file");
            return Err(Error::Io(e));
        }
    };

    // Get encoding
    let mut encoding = UTF_8;
    if let Some(enc) = args.encoding {
        encoding = if let Some(enc) = Encoding::for_label(enc.as_str().as_bytes()) {
            enc
        } else {
            eprintln!("Invalid encoding provided");
            return Err(Error::EncodingError);
        }
    }

    println!(
        "Archive: {:?}\nEncoding: {}",
        zip_path.file_name().unwrap(),
        encoding.name()
    );

    // Parse zip file
    let mut archive = match ZipArchive::new(zip) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("Unable to parse zip file");
            return Err(Error::Zip(e));
        }
    };

    // Set output destination
    let mut destination = Path::new(zip_path.file_stem().unwrap()).to_path_buf();
    if args.smart {
        destination = PathBuf::from(zip_path.parent().unwrap().to_str().unwrap());
        destination.push(uuid::Uuid::new_v4().to_string())
    } else if let Some(out) = args.out {
        let output_meta = match fs::metadata(&out) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Invalid output directory");
                return Err(Error::Io(e));
            }
        };
        if output_meta.is_file() {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Output directory is not a directory",
            )));
        }
        destination = out;
    }

    let num_digits = |n| successors(Some(n), |&n| (n >= 10).then_some(n / 10)).count();
    let archive_digits = num_digits(archive.len()) + 2;

    for i in 0..archive.len() {
        let mut file: ZipFile;
        if let Some(password) = &args.password {
            file = archive.by_index_decrypt(i, password.as_bytes())??;
        } else {
            file = archive.by_index(i)?;
        }
        let outpath = match inflate(&mut file, &destination, encoding) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Unable to extract file {}", file.name());
                return Err(Error::Io(e));
            }
        };

        // TODO print before inflating (since this currently being printed *after* inflating, it's misleading).
        if !args.silent {
            {
                let comment = file.comment();
                if !comment.is_empty() {
                    println!("{i:>archive_digits$} comment:   {comment}");
                }
            }
            if outpath.is_dir() {
                println!("{i:>archive_digits$} creating:  \"{}\"", outpath.display());
            } else {
                println!(
                    "{i:>archive_digits$} inflating: \"{}\" ({} bytes)",
                    outpath.display(),
                    file.size()
                );
            }
        }
    }

    if args.smart {
        // Check if the directory only contains folders
        process_directory(
            &mut destination,
            zip_path.file_stem().unwrap().to_str().unwrap(),
            args.force,
        )
        .expect("Unable to smart process output directory");
    }

    // TODO add byte units
    println!(
        "Extracted {} files to \"{}\"",
        archive.len(),
        fs::canonicalize(destination).unwrap().display()
    );
    Ok(())
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

fn process_directory(path: &mut PathBuf, zip_name: &str, force: bool) -> io::Result<()> {
    // Check if the directory only contains folders
    let mut contains_only_dirs = true;

    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if !file_type.is_dir() {
            contains_only_dirs = false;
            break;
        }
    }

    if contains_only_dirs {
        // Move contents to parent and delete the directory
        move_dir_contents_to_parent(path, force)?;
    } else {
        // Rename the directory to the zip file name
        let new_path = path.with_file_name(zip_name);
        if force && fs::exists(&new_path)? {
            fs::remove_dir_all(&new_path)?;
        }
        fs::rename(&path, &new_path)?;
        *path = new_path
    }

    Ok(())
}

fn move_dir_contents_to_parent(dir: &mut PathBuf, force: bool) -> io::Result<()> {
    let parent_dir = dir.parent().unwrap(); // Get the parent directory of the folder
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();

        // Move each entry (file/folder) to the parent directory
        let file_name = path.file_name().unwrap();
        let new_path = parent_dir.join(file_name);
        if force && fs::exists(&new_path)? {
            fs::remove_dir_all(&new_path)?;
        }
        fs::rename(&path, &new_path)?;
    }

    // Remove the original directory
    fs::remove_dir(&dir)?;
    *dir = parent_dir.to_path_buf();
    Ok(())
}

// TODO: create Byte enum and impl display with 3 sig figs
//  0-999 Bytes, 1.00-999KB, etc.
