use encoding_rs::{Encoding, UTF_8};
use std::iter::successors;
use std::path::{Path, PathBuf};
use std::{fs, io};
use zip::read::ZipFile;
use zip::ZipArchive;

pub fn unzip(
    file: &Path,
    destination: &Path,
    silent: bool,
    password: Option<String>,
    encoding: Option<String>,
) -> Result<u64, crate::Error> {
    let zip = match fs::File::open(file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Unable to open zip file");
            return Err(crate::Error::Io(e));
        }
    };

    // Get encoding
    let mut use_encoding = UTF_8;
    if let Some(enc) = encoding {
        use_encoding = if let Some(enc) = Encoding::for_label(enc.as_bytes()) {
            enc
        } else {
            eprintln!("Invalid encoding provided");
            return Err(crate::Error::EncodingError);
        }
    }

    println!(
        "Archive: {:?}\nEncoding: {}",
        file.file_name().unwrap(),
        use_encoding.name()
    );

    // Parse zip file
    let mut archive = match ZipArchive::new(zip) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("Unable to parse zip file");
            return Err(crate::Error::Zip(e));
        }
    };

    let num_digits = |n| successors(Some(n), |&n| (n >= 10).then_some(n / 10)).count();
    let archive_digits = num_digits(archive.len()) + 2;

    for i in 0..archive.len() {
        let mut file: ZipFile;
        if let Some(password) = &password {
            file = archive.by_index_decrypt(i, password.as_bytes())??;
        } else {
            file = archive.by_index(i)?;
        }
        let outpath = match inflate(&mut file, destination, use_encoding) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Unable to extract file {}", file.name());
                return Err(crate::Error::Io(e));
            }
        };

        // TODO print before inflating (since this currently being printed *after* inflating, it's misleading).
        if !silent {
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
    Ok(archive.len() as u64)
}

fn inflate(
    file: &mut ZipFile,
    destination: &Path,
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
