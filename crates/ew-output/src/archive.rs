//! ZIP archives of generated files (the Gerber package for the board
//! maker, the parts of an XLSX workbook).

use std::io::{Cursor, Write as _};

use zip::write::SimpleFileOptions;

use crate::OutputFile;

/// Packs files into a ZIP archive. Entries keep their order; their time
/// stamp is fixed (1 January 1980) so that the same files give the same
/// archive.
#[must_use]
pub fn zip(files: &[OutputFile]) -> Vec<u8> {
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        for file in files {
            if writer.start_file(file.name.as_str(), options).is_err()
                || writer.write_all(&file.data).is_err()
            {
                break;
            }
        }
        let _ = writer.finish();
    }
    buffer.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    #[test]
    fn packs_files() {
        let bytes = zip(&[
            OutputFile::text("a.txt", "hello".into()),
            OutputFile::text("dir/b.txt", "world".into()),
        ]);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 2);
        let mut text = String::new();
        archive
            .by_name("dir/b.txt")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "world");
    }
}
