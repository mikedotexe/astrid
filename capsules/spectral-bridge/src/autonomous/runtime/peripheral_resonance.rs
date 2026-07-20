mod peripheral_resonance {
    use std::fs::File;
    use std::io::{self, Read};
    use std::path::Path;

    const PREVIEW_CHARS: usize = 200;

    pub(super) fn read_preview(path: &Path) -> io::Result<String> {
        read_utf8_preview(File::open(path)?, PREVIEW_CHARS)
    }

    fn read_utf8_preview<R: Read>(mut reader: R, max_chars: usize) -> io::Result<String> {
        let mut preview = String::new();
        let mut first = [0_u8; 1];

        for _ in 0..max_chars {
            if reader.read(&mut first)? == 0 {
                break;
            }

            let width = match first[0] {
                0x00..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => return Err(invalid_utf8()),
            };
            let mut bytes = [0_u8; 4];
            bytes[0] = first[0];
            if width > 1 {
                reader.read_exact(&mut bytes[1..width])?;
            }
            let scalar = std::str::from_utf8(&bytes[..width]).map_err(|_| invalid_utf8())?;
            preview.push_str(scalar);
        }

        Ok(preview)
    }

    fn invalid_utf8() -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, "peripheral resonance is not UTF-8")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        struct HugeAsciiReader {
            remaining: usize,
            bytes_read: usize,
        }

        impl Read for HugeAsciiReader {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if self.remaining == 0 || output.is_empty() {
                    return Ok(0);
                }
                let count = output.len().min(self.remaining);
                output[..count].fill(b'x');
                self.remaining -= count;
                self.bytes_read += count;
                Ok(count)
            }
        }

        #[test]
        fn bounded_preview_matches_the_existing_first_200_character_view() {
            let source = format!(
                "{}{}",
                "warmth-λ-".repeat(80),
                "content beyond the preview remains unread"
            );
            let expected: String = source.chars().take(PREVIEW_CHARS).collect();

            let actual = read_utf8_preview(source.as_bytes(), PREVIEW_CHARS).unwrap();

            assert_eq!(actual, expected);
        }

        #[test]
        fn fifty_megabyte_source_reads_only_the_requested_ascii_prefix() {
            let mut source = HugeAsciiReader {
                remaining: 50 * 1024 * 1024,
                bytes_read: 0,
            };

            let preview = read_utf8_preview(&mut source, PREVIEW_CHARS).unwrap();

            assert_eq!(preview, "x".repeat(PREVIEW_CHARS));
            assert_eq!(source.bytes_read, PREVIEW_CHARS);
            assert!(source.remaining > 49 * 1024 * 1024);
        }

        #[test]
        fn four_byte_scalars_stop_at_the_exact_character_boundary() {
            let source = "🜂".repeat(PREVIEW_CHARS + 1);

            let preview = read_utf8_preview(source.as_bytes(), PREVIEW_CHARS).unwrap();

            assert_eq!(preview, "🜂".repeat(PREVIEW_CHARS));
            assert_eq!(preview.len(), PREVIEW_CHARS * 4);
        }

        #[test]
        fn malformed_utf8_before_the_boundary_is_rejected() {
            let source = [b'a', 0xf0, 0x28, 0x8c, 0x28];

            let error = read_utf8_preview(source.as_slice(), PREVIEW_CHARS).unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }
}
