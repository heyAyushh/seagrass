use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

pub(crate) const MAX_PROJECT_FILE_BYTES: u64 = 512 * 1024;

pub(crate) fn read_limited_text(path: &Path) -> io::Result<Option<String>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_PROJECT_FILE_BYTES {
        return Ok(None);
    }

    let mut text = String::new();
    File::open(path)?.read_to_string(&mut text)?;
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERSIZED_TEST_BYTES: usize = MAX_PROJECT_FILE_BYTES as usize + 1;

    #[test]
    fn read_limited_text_rejects_oversized_files() {
        let path = std::env::temp_dir().join(format!(
            "seagrass-large-file-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, vec![b'a'; OVERSIZED_TEST_BYTES]).unwrap();

        let result = read_limited_text(&path).unwrap();

        assert!(result.is_none());
        let _ = fs::remove_file(path);
    }
}
