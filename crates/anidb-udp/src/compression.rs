//! Decompression of datagrams sent after AUTH with `comp=1`. The compression is
//! described in the specification, see the crate root for the link.

use flate2::{Decompress, FlushDecompress, Status};

use crate::error::Error;

/// Output space added each time the buffer runs full.
const CHUNK: usize = 4096;

/// A compressed datagram starts with two zero bytes, followed by a zlib stream.
#[must_use]
pub fn is_compressed(datagram: &[u8]) -> bool {
    matches!(datagram, [0, 0, ..])
}

/// Inflates a compressed datagram.
///
/// With `comp=1` in AUTH the server deflates a reply that would not fit one
/// datagram, with zlib, instead of truncating it. Datagrams are at most 1400
/// bytes.
///
/// The stream is inflated with sync flush semantics, so a stream cut short at
/// the end of the datagram returns the prefix that could be decoded, without an
/// error. Corrupt data is an error.
///
/// # Errors
///
/// Returns [`Error::Decompress`] when the data is not valid deflate.
pub fn decompress(datagram: &[u8]) -> Result<Vec<u8>, Error> {
    let input = datagram.get(2..).unwrap_or_default();
    let mut inflater = Decompress::new(true);
    let mut output = Vec::with_capacity(input.len().saturating_mul(4).max(CHUNK));
    loop {
        if output.len() == output.capacity() {
            output.reserve(CHUNK);
        }
        // the backend may hold decoded bytes internally, so keep going until a
        // call neither reads input nor writes output
        let consumed = usize::try_from(inflater.total_in()).unwrap_or(input.len());
        let output_len = output.len();
        let status =
            inflater.decompress_vec(&input[consumed..], &mut output, FlushDecompress::Sync)?;
        if matches!(status, Status::StreamEnd) {
            break;
        }
        let consumed_after = usize::try_from(inflater.total_in()).unwrap_or(input.len());
        if consumed_after == consumed && output.len() == output_len {
            break;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    // zlib stream of `plain()`, made with an independent zlib encoder, behind two zero bytes
    const ZLIB_HEX: &str = "789c35d23d4ec6301084e1dea7c81132b3b19394fc154850414349f1212101b50b1f1e4d76dcbed53cdabdbb7f787c5a482caf1f2fcf6fef651d5fdf3fb7655dfe3e7f6f631dbd200bb2eca3176661166ca397c8149988d1cb9669733a462f3553cd1475f4d232b54c1b472f7ba6dde91cbd1c998e4cb58d5ece4c67a6165aeaf1f0fafd9a3ff74f80043001361c32c00858714a013360c729070c81255845812d30069006e6c01e402058049340996014ac428845b3681636b96817edc2769d66de661ea74a46cb68199a68348da6a1c946db386dbb6cb48dd376c846db386d876cb48dd376ca46db681b57d9c2b6b08d902d6c0bdb08d9c2b6b08dbc3e6fbedefcbd902d6c8bf97e215bd816b671932d6c0bdb58650bdbc23656d9c2b6b08d4db6b02da66d8fd1ff0193b90f9e";

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn plain() -> String {
        let lines: Vec<String> = (0..40)
            .map(|i| format!("{i}|file {i} name|{}|x", i * 7))
            .collect();
        format!("ABCDE 221 MYLIST\n{}", lines.join("\n"))
    }

    fn datagram() -> Vec<u8> {
        let mut bytes = vec![0, 0];
        bytes.extend(hex(ZLIB_HEX));
        bytes
    }

    #[test]
    fn detects_the_two_zero_byte_prefix() {
        assert!(is_compressed(&[0, 0, 0x78]));
        assert!(is_compressed(&[0, 0]));
        assert!(!is_compressed(&[0]));
        assert!(!is_compressed(&[]));
        assert!(!is_compressed(&[0, 1, 0x78]));
        assert!(!is_compressed(b"ABCDE 200"));
    }

    #[test]
    fn inflates_a_full_stream() {
        assert_eq!(decompress(&datagram()).unwrap(), plain().as_bytes());
    }

    // this datagram cut 10 bytes short still gives 846 bytes
    #[test]
    fn truncated_stream_returns_the_prefix() {
        let mut bytes = datagram();
        bytes.truncate(bytes.len() - 10);
        let out = decompress(&bytes).unwrap();
        assert_eq!(out.len(), 846);
        assert!(plain().as_bytes().starts_with(&out));
    }

    // several times the initial output buffer, so the buffer has to grow
    #[test]
    fn inflates_output_larger_than_one_chunk() {
        use std::io::Write as _;

        let data: Vec<u8> = (0..200_000u32)
            .scan(0x2545_f491_u32, |state, _| {
                *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                Some(u8::try_from((*state >> 16) & 0xff).unwrap_or_default())
            })
            .collect();
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&data).unwrap();
        let mut datagram = vec![0, 0];
        datagram.extend(encoder.finish().unwrap());

        assert_eq!(decompress(&datagram).unwrap(), data);

        datagram.truncate(datagram.len() - 10);
        let out = decompress(&datagram).unwrap();
        assert!(out.len() > CHUNK * 4 && out.len() < data.len());
        assert!(data.starts_with(&out));
    }

    #[test]
    fn empty_stream_is_empty() {
        assert_eq!(decompress(&[0, 0]).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn corrupt_stream_is_an_error() {
        // zlib header, then a block with the reserved type 3
        let bytes = [0, 0, 0x78, 0x9c, 0xff, 0xff, 0xff];
        assert!(matches!(decompress(&bytes), Err(Error::Decompress(_))));
    }
}
