//! Stream filters: Flate, LZW, `ASCIIHex`, ASCII85 and `RunLength`, with the
//! PNG and TIFF predictors. Image filters (DCT, JPX, CCITT, JBIG2) are left
//! encoded; the reader only needs their bounds.

use std::io::Read as _;

use flate2::read::{DeflateDecoder, ZlibDecoder};

use super::object::{Dict, Object};

/// Largest decoded stream (protects against decompression bombs).
const MAX_DECODED: usize = 256 * 1024 * 1024;

/// Decodes `data` with the filters of a stream dictionary. Decoding stops
/// at the first image filter. Broken data decodes as far as it goes.
#[must_use]
pub fn decode(dict: &Dict, data: &[u8]) -> Vec<u8> {
    let filters = list(
        dict.get(b"Filter".as_slice())
            .or_else(|| dict.get(b"F".as_slice())),
    );
    let params_list = dict
        .get(b"DecodeParms".as_slice())
        .or_else(|| dict.get(b"DP".as_slice()));
    let mut out = data.to_vec();
    for (index, filter) in filters.iter().enumerate() {
        let params = match params_list {
            Some(Object::Array(items)) => items.get(index).and_then(Object::dict),
            Some(Object::Dict(d)) if index == 0 => Some(d),
            _ => None,
        };
        out = match filter.as_slice() {
            b"FlateDecode" | b"Fl" => predict(&inflate(&out), params),
            b"LZWDecode" | b"LZW" => {
                let early = params
                    .and_then(|p| p.get(b"EarlyChange".as_slice()))
                    .and_then(Object::int)
                    .unwrap_or(1);
                predict(&lzw(&out, early != 0), params)
            }
            b"ASCIIHexDecode" | b"AHx" => ascii_hex(&out),
            b"ASCII85Decode" | b"A85" => ascii85(&out),
            b"RunLengthDecode" | b"RL" => run_length(&out),
            b"Crypt" => out,
            _ => return out,
        };
    }
    out
}

/// Whether the stream has an image filter (its data is not text).
#[must_use]
pub fn is_image_filter(name: &[u8]) -> bool {
    matches!(
        name,
        b"DCTDecode" | b"DCT" | b"JPXDecode" | b"CCITTFaxDecode" | b"CCF" | b"JBIG2Decode"
    )
}

fn list(filter: Option<&Object>) -> Vec<Vec<u8>> {
    match filter {
        Some(Object::Name(n)) => vec![n.clone()],
        Some(Object::Array(items)) => items
            .iter()
            .filter_map(|o| o.name().map(<[u8]>::to_vec))
            .collect(),
        _ => Vec::new(),
    }
}

/// Inflates zlib data, falling back to raw deflate; keeps what decodes
/// before an error.
#[must_use]
pub fn inflate(data: &[u8]) -> Vec<u8> {
    fn read_all(mut reader: impl std::io::Read) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return (out, true),
                Ok(n) => {
                    out.extend_from_slice(&buffer[..n]);
                    if out.len() > MAX_DECODED {
                        return (out, true);
                    }
                }
                Err(_) => return (out, false),
            }
        }
    }
    let (out, ok) = read_all(ZlibDecoder::new(data));
    if ok || !out.is_empty() {
        return out;
    }
    let (raw, _) = read_all(DeflateDecoder::new(data));
    if raw.is_empty() {
        // Some writers put two stray bytes before a raw stream.
        let mut tail = Vec::new();
        let _ = DeflateDecoder::new(data.get(2..).unwrap_or_default()).read_to_end(&mut tail);
        return tail;
    }
    raw
}

fn predict(data: &[u8], params: Option<&Dict>) -> Vec<u8> {
    let Some(params) = params else {
        return data.to_vec();
    };
    let get = |key: &[u8], default: i64| {
        params
            .get(key)
            .and_then(Object::int)
            .unwrap_or(default)
            .max(0) as usize
    };
    let predictor = get(b"Predictor", 1);
    if predictor < 2 {
        return data.to_vec();
    }
    let colors = get(b"Colors", 1).clamp(1, 32);
    let bits = get(b"BitsPerComponent", 8).clamp(1, 16);
    let columns = get(b"Columns", 1).clamp(1, 1 << 20);
    let bpp = (colors * bits).div_ceil(8).max(1);
    let row = (colors * bits * columns).div_ceil(8);
    if predictor == 2 {
        // TIFF predictor, 8-bit components only.
        let mut out = data.to_vec();
        if bits == 8 {
            for line in out.chunks_mut(row) {
                for i in bpp..line.len() {
                    line[i] = line[i].wrapping_add(line[i - bpp]);
                }
            }
        }
        return out;
    }
    let mut out = Vec::with_capacity(data.len());
    let mut previous = vec![0u8; row];
    for chunk in data.chunks(row + 1) {
        let Some((&kind, line)) = chunk.split_first() else {
            break;
        };
        let mut current = line.to_vec();
        current.resize(row, 0);
        for i in 0..row {
            let left = if i >= bpp { current[i - bpp] } else { 0 };
            let up = previous[i];
            let corner = if i >= bpp { previous[i - bpp] } else { 0 };
            current[i] = match kind {
                1 => current[i].wrapping_add(left),
                2 => current[i].wrapping_add(up),
                3 => current[i].wrapping_add(u16::midpoint(u16::from(left), u16::from(up)) as u8),
                4 => current[i].wrapping_add(paeth(left, up, corner)),
                _ => current[i],
            };
        }
        out.extend_from_slice(&current[..line.len().min(row)]);
        previous = current;
    }
    out
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let pa = (p - i16::from(a)).abs();
    let pb = (p - i16::from(b)).abs();
    let pc = (p - i16::from(c)).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// LZW with 9 to 12 bit codes, as in PDF and TIFF.
fn lzw(data: &[u8], early: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut table: Vec<Vec<u8>> = Vec::new();
    let reset = |table: &mut Vec<Vec<u8>>| {
        table.clear();
        for i in 0..=255u8 {
            table.push(vec![i]);
        }
        table.push(Vec::new()); // 256 clear
        table.push(Vec::new()); // 257 end
    };
    reset(&mut table);
    let mut width = 9u32;
    let mut buffer = 0u32;
    let mut bits = 0u32;
    let mut previous: Option<Vec<u8>> = None;
    for &byte in data {
        buffer = buffer << 8 | u32::from(byte);
        bits += 8;
        while bits >= width {
            let code = (buffer >> (bits - width)) & ((1 << width) - 1);
            bits -= width;
            buffer &= (1 << bits) - 1;
            match code {
                256 => {
                    reset(&mut table);
                    width = 9;
                    previous = None;
                    continue;
                }
                257 => return out,
                _ => {}
            }
            let code = code as usize;
            let entry = if code < table.len() {
                table[code].clone()
            } else if let Some(p) = &previous {
                let mut e = p.clone();
                e.push(p[0]);
                e
            } else {
                return out;
            };
            out.extend_from_slice(&entry);
            if out.len() > MAX_DECODED {
                return out;
            }
            if let Some(p) = previous.take() {
                let mut added = p;
                added.push(entry[0]);
                table.push(added);
            }
            previous = Some(entry);
            let limit = table.len() + usize::from(early);
            width = match limit {
                0..=511 => 9,
                512..=1023 => 10,
                1024..=2047 => 11,
                _ => 12,
            };
        }
    }
    out
}

fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2);
    let mut high: Option<u8> = None;
    for &c in data {
        if c == b'>' {
            break;
        }
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => continue,
        };
        if let Some(h) = high.take() {
            out.push(h << 4 | v);
        } else {
            high = Some(v);
        }
    }
    if let Some(h) = high {
        out.push(h << 4);
    }
    out
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4 / 5);
    let mut group = [0u8; 5];
    let mut n = 0;
    let body = data.strip_prefix(b"<~").unwrap_or(data);
    for &c in body {
        match c {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                group[n] = c - b'!';
                n += 1;
                if n == 5 {
                    let value = group.iter().fold(0u64, |v, &d| v * 85 + u64::from(d));
                    out.extend_from_slice(&(value as u32).to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for slot in group.iter_mut().skip(n) {
            *slot = 84;
        }
        let value = group.iter().fold(0u64, |v, &d| v * 85 + u64::from(d));
        out.extend_from_slice(&(value as u32).to_be_bytes()[..n - 1]);
    }
    out
}

fn run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let n = data[i];
        i += 1;
        match n {
            128 => break,
            0..=127 => {
                let end = (i + usize::from(n) + 1).min(data.len());
                out.extend_from_slice(&data[i..end]);
                i = end;
            }
            _ => {
                if let Some(&b) = data.get(i) {
                    out.extend(std::iter::repeat_n(b, 257 - usize::from(n)));
                }
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    fn dict(entries: &[(&str, Object)]) -> Dict {
        entries
            .iter()
            .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
            .collect()
    }

    #[test]
    fn decodes_flate_with_png_predictor() {
        // Two rows of 3 bytes with the "up" filter.
        let raw = [2u8, 1, 2, 3, 2, 1, 1, 1];
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap_or_default();
        let compressed = encoder.finish().unwrap_or_default();
        let params = Object::Dict(dict(&[
            ("Predictor", Object::Int(12)),
            ("Columns", Object::Int(3)),
        ]));
        let d = dict(&[
            ("Filter", Object::Name(b"FlateDecode".to_vec())),
            ("DecodeParms", params),
        ]);
        assert_eq!(decode(&d, &compressed), vec![1, 2, 3, 2, 3, 4]);
    }

    #[test]
    fn decodes_ascii_filters() {
        assert_eq!(ascii_hex(b"48 65 6c6C6f>"), b"Hello");
        assert_eq!(ascii85(b"<~87cURD]i,\"Ebo80~>"), b"Hello World!");
        assert_eq!(ascii85(b"z~>"), vec![0, 0, 0, 0]);
        assert_eq!(
            run_length(&[2, b'a', b'b', b'c', 254, b'x', 128]),
            b"abcxxx"
        );
    }

    #[test]
    fn decodes_lzw() {
        // The example of the PDF reference: 45 45 45 45 45 65 45 45 45 66.
        let encoded = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        assert_eq!(
            lzw(&encoded, true),
            vec![45, 45, 45, 45, 45, 65, 45, 45, 45, 66]
        );
    }

    #[test]
    fn keeps_the_decoded_part_of_broken_flate_data() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&b"BT /F1 12 Tf (Hello) Tj ET ".repeat(200))
            .unwrap_or_default();
        let mut compressed = encoder.finish().unwrap_or_default();
        compressed.truncate(compressed.len() - 6);
        let out = inflate(&compressed);
        assert!(out.starts_with(b"BT /F1 12 Tf (Hello) Tj ET"));
    }
}
