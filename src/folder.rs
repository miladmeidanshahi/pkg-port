use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

pub struct Inspection {
    pub path: PathBuf,
    pub files: HashMap<String, u64>,
}

pub fn inspect(path: PathBuf) -> Receiver<Result<Inspection, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = scan(&path).map(|files| Inspection { path, files });
        let _ = sender.send(result);
    });
    receiver
}

pub fn matching_parts(first_url: &str, count: usize, files: &HashMap<String, u64>) -> usize {
    let Some(filename) = first_url.rsplit('/').next() else {
        return 0;
    };
    let Some(prefix) = filename.strip_suffix("_0.pkg") else {
        return 0;
    };
    (0..count)
        .filter(|index| files.contains_key(&format!("{prefix}_{index}.pkg")))
        .count()
}

fn scan(path: &Path) -> Result<HashMap<String, u64>, String> {
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;
    let mut files = HashMap::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".pkg") {
            let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            files.insert(name, size);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::{matching_parts, scan};
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn scan_counts_regular_pkg_files_only() {
        let path =
            std::env::temp_dir().join(format!("ps-rusty-folder-test-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("part_0.pkg"), b"pkg").unwrap();
        fs::write(path.join("notes.txt"), b"ignore").unwrap();
        let files = scan(&path).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files["part_0.pkg"], 3);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn matching_parts_requires_original_filenames() {
        let files = HashMap::from([("game_0.pkg".into(), 1), ("game_2.pkg".into(), 1)]);
        assert_eq!(
            matching_parts("http://example.com/game_0.pkg", 3, &files),
            2
        );
        assert_eq!(
            matching_parts("http://example.com/game_0.pkg", 0, &files),
            0
        );
    }
}
