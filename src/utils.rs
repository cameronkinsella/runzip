use std::path::PathBuf;
use std::{fs, io};

pub fn process_directory(path: &mut PathBuf, zip_name: &str, force: bool) -> io::Result<()> {
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
