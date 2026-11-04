use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::core::ingest::Acquired;
use crate::core::limits;
use crate::error::IngestError;

pub fn read_file(path: &Path) -> Result<Acquired, IngestError> {
    let io_error = |source| IngestError::Io {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(IngestError::NotAFile {
            path: path.to_owned(),
        });
    }
    if metadata.len() > limits::encoded() {
        return Err(IngestError::InputTooLarge {
            size: metadata.len(),
            limit: limits::encoded(),
        });
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limits::encoded().saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > limits::encoded() {
        return Err(IngestError::InputTooLarge {
            size: bytes.len() as u64,
            limit: limits::encoded(),
        });
    }
    Ok(Acquired::Encoded(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_regular_files() {
        let path = std::env::temp_dir().join("komaru_read_file_regular.bin");
        std::fs::write(&path, [1u8, 2, 3]).unwrap();
        let result = read_file(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(result, Ok(Acquired::Encoded(bytes)) if bytes == [1, 2, 3]));
    }

    #[test]
    fn rejects_directories() {
        assert!(matches!(
            read_file(&std::env::temp_dir()),
            Err(IngestError::NotAFile { .. }) | Err(IngestError::Io { .. })
        ));
    }

    #[test]
    fn reports_missing_files() {
        let path = std::env::temp_dir().join("komaru_read_file_missing.bin");
        assert!(matches!(read_file(&path), Err(IngestError::Io { .. })));
    }
}
