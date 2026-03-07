use crc32fast::Hasher as Crc32Hasher;
use md5::{Digest, Md5};
use std::io::{self, Read};
use std::path::Path;

/// Compute MD5 hash of a file with progress callback.
///
/// The callback receives `(processed_bytes, total_bytes)`.
pub fn md5_file_with_progress<F>(path: &Path, mut on_progress: F) -> io::Result<String>
where
    F: FnMut(u64, u64),
{
    let mut file = std::fs::File::open(path)?;
    let total_bytes = file.metadata()?.len();
    let mut processed_bytes = 0u64;
    let mut hasher = Md5::new();
    let mut buffer = [0u8; 256 * 1024];

    on_progress(0, total_bytes);

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        processed_bytes += bytes_read as u64;
        on_progress(processed_bytes, total_bytes);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute MD5 hash of a file
pub fn md5_file(path: &Path) -> io::Result<String> {
    md5_file_with_progress(path, |_processed, _total| {})
}

/// Compute CRC32 hash of a file
pub fn crc32_file(path: &Path) -> io::Result<u32> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Crc32Hasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Compute CRC32 hash of bytes
pub fn crc32_bytes(data: &[u8]) -> u32 {
    let mut hasher = Crc32Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_md5_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "hello world").unwrap();
        temp_file.flush().unwrap();

        let hash = md5_file(temp_file.path()).unwrap();
        // MD5 of "hello world" is 5eb63bbbe01eeed093cb22bb8f5acdc3
        assert_eq!(hash, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_crc32_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "hello world").unwrap();
        temp_file.flush().unwrap();

        let hash = crc32_file(temp_file.path()).unwrap();
        // CRC32 of "hello world" is 0x0d4a1185 (222957061 in decimal)
        assert_eq!(hash, 0x0d4a1185);
    }

    #[test]
    fn test_crc32_bytes() {
        let data = b"hello world";
        let hash = crc32_bytes(data);
        assert_eq!(hash, 0x0d4a1185);
    }

    #[test]
    fn test_md5_large_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write 1MB of data to test streaming
        let data = vec![0u8; 1024 * 1024];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let hash = md5_file(temp_file.path()).unwrap();
        // MD5 of 1MB of zeros
        assert_eq!(hash, "b6d81b360a5672d80c27430f39153e2c");
    }
}
