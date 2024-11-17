use std::fs::File;
use std::io::Write;
use std::path::Path;
use unrar::Archive;

pub fn unrar(
    file: &Path,
    destination: &Path,
    silent: bool,
    password: Option<String>,
) -> Result<u64, crate::Error> {
    let mut archive = if let Some(pwd) = password {
        Archive::with_password(&file, &pwd).open_for_processing()?
    } else {
        Archive::new(&file).open_for_processing()?
    };
    let mut file_count = 0;

    while let Some(header) = archive.read_header()? {
        let filename = header.entry().filename.clone();

        archive = if header.entry().is_file() {
            let outpath = destination.join(filename);
            if !silent {
                if outpath.is_dir() {
                    println!("creating:  \"{}\"", outpath.display());
                } else {
                    println!(
                        "inflating: \"{}\" ({} bytes)",
                        outpath.display(),
                        header.entry().unpacked_size,
                    );
                }
            }

            let (data, cursor) = header.read()?;
            std::fs::create_dir_all(outpath.parent().unwrap()).unwrap();
            let mut output_file = File::create(&outpath).unwrap();
            output_file.write_all(&data).unwrap();
            file_count += 1;
            cursor
        } else {
            header.skip()?
        };
    }
    Ok(file_count)
}
