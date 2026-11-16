//! A small PDF writer for synthetic datasheets and tests: plain or
//! compressed streams, object streams with a cross-reference stream, and
//! RC4 or AES-128 encryption with the empty user password — the variants
//! real datasheets come in.

use std::fmt::Write as _;
use std::io::Write as _;
use std::rc::Rc;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use super::crypt::{aes_encrypt, object_key, rc4, standard_r3};
use super::object::{Dict, Object, Stream, is_delimiter, is_white};

/// Encryption of a written file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encryption {
    /// 128-bit RC4 (revision 3).
    Rc4,
    /// AES-128 (revision 4).
    Aes128,
}

/// How a file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteOptions {
    /// Compress streams with Flate.
    pub compress: bool,
    /// Put objects into an object stream and write a cross-reference
    /// stream (PDF 1.5).
    pub object_streams: bool,
    /// Encrypt with the empty user password.
    pub encryption: Option<Encryption>,
}

/// Builds a PDF file object by object.
#[derive(Debug, Default)]
pub struct Writer {
    objects: Vec<Option<Object>>,
    options: WriteOptions,
}

/// A dictionary from `(key, value)` pairs.
#[must_use]
pub fn dict(entries: Vec<(&str, Object)>) -> Dict {
    entries
        .into_iter()
        .map(|(k, v)| (k.as_bytes().to_vec(), v))
        .collect()
}

/// A name object.
#[must_use]
pub fn name(value: &str) -> Object {
    Object::Name(value.as_bytes().to_vec())
}

/// An array of numbers.
#[must_use]
pub fn numbers(values: &[f64]) -> Object {
    Object::Array(
        values
            .iter()
            .map(|v| {
                if v.fract() == 0.0 && v.abs() < 1e15 {
                    Object::Int(*v as i64)
                } else {
                    Object::Real(*v)
                }
            })
            .collect(),
    )
}

/// A reference object.
#[must_use]
pub const fn reference(number: u32) -> Object {
    Object::Ref(number, 0)
}

fn real(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn write_name(out: &mut Vec<u8>, value: &[u8]) {
    out.push(b'/');
    for &b in value {
        if !(0x21..=0x7e).contains(&b) || b == b'#' || is_delimiter(b) || is_white(b) {
            let _ = write!(out, "#{b:02X}");
        } else {
            out.push(b);
        }
    }
}

type Crypt<'a> = Option<(&'a [u8], Encryption, u32)>;

fn encrypt(crypt: Crypt<'_>, data: &[u8]) -> Vec<u8> {
    match crypt {
        None => data.to_vec(),
        Some((key, Encryption::Rc4, n)) => rc4(&object_key(key, n, 0, false), data),
        Some((key, Encryption::Aes128, n)) => {
            // A fixed vector keeps files reproducible.
            let iv: [u8; 16] = std::array::from_fn(|i| (i as u8).wrapping_mul(17) ^ (n as u8));
            aes_encrypt(&object_key(key, n, 0, true), &iv, data)
        }
    }
}

/// Serializes an object.
pub fn serialize(object: &Object, out: &mut Vec<u8>, crypt: Crypt<'_>) {
    match object {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Int(v) => out.extend_from_slice(v.to_string().as_bytes()),
        Object::Real(v) => out.extend_from_slice(real(*v).as_bytes()),
        Object::Name(n) => write_name(out, n),
        Object::String(s) => {
            let data = encrypt(crypt, s);
            if crypt.is_some() || data.iter().any(|b| *b < 0x20 || *b > 0x7e) {
                out.push(b'<');
                for b in data {
                    let _ = write!(out, "{b:02X}");
                }
                out.push(b'>');
            } else {
                out.push(b'(');
                for b in data {
                    if matches!(b, b'(' | b')' | b'\\') {
                        out.push(b'\\');
                    }
                    out.push(b);
                }
                out.push(b')');
            }
        }
        Object::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                serialize(item, out, crypt);
            }
            out.push(b']');
        }
        Object::Dict(d) => serialize_dict(d, out, crypt),
        Object::Stream(s) => {
            let data = encrypt(crypt, &s.data);
            let mut d = s.dict.clone();
            d.insert(b"Length".to_vec(), Object::Int(data.len() as i64));
            serialize_dict(&d, out, crypt);
            out.extend_from_slice(b"\nstream\n");
            out.extend_from_slice(&data);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Ref(n, g) => {
            let _ = write!(out, "{n} {g} R");
        }
    }
}

fn serialize_dict(d: &Dict, out: &mut Vec<u8>, crypt: Crypt<'_>) {
    out.extend_from_slice(b"<<");
    for (k, v) in d {
        write_name(out, k);
        out.push(b' ');
        serialize(v, out, crypt);
    }
    out.extend_from_slice(b">>");
}

/// Compresses data with zlib.
#[must_use]
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = encoder.write_all(data);
    encoder.finish().unwrap_or_default()
}

impl Writer {
    /// A writer with options.
    #[must_use]
    pub fn new(options: WriteOptions) -> Self {
        Self {
            objects: vec![None],
            options,
        }
    }

    /// Reserves an object number.
    pub fn reserve(&mut self) -> u32 {
        self.objects.push(None);
        (self.objects.len() - 1) as u32
    }

    /// Sets a reserved object.
    pub fn set(&mut self, number: u32, object: Object) {
        if let Some(slot) = self.objects.get_mut(number as usize) {
            *slot = Some(object);
        }
    }

    /// Adds an object.
    pub fn add(&mut self, object: Object) -> u32 {
        let n = self.reserve();
        self.set(n, object);
        n
    }

    /// Adds a stream, compressed when the options say so.
    pub fn stream(&mut self, mut d: Dict, data: &[u8]) -> u32 {
        let data = if self.options.compress {
            d.insert(b"Filter".to_vec(), name("FlateDecode"));
            deflate(data)
        } else {
            data.to_vec()
        };
        self.add(Object::Stream(Stream {
            dict: d,
            data: Rc::new(data),
        }))
    }

    /// The file, with `root` as catalog and `info` as document
    /// information.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "the file layout in one place")]
    pub fn finish(mut self, root: u32, info: Option<u32>) -> Vec<u8> {
        let id0: Vec<u8> = b"ElectronicWerkDS".to_vec();
        let encryption = self.options.encryption.map(|e| {
            let (o, u, key) = standard_r3(b"owner", &id0, -3904);
            let mut d = dict(vec![
                ("Filter", name("Standard")),
                ("O", Object::String(o)),
                ("U", Object::String(u)),
                ("P", Object::Int(-3904)),
                ("Length", Object::Int(128)),
            ]);
            match e {
                Encryption::Rc4 => {
                    d.insert(b"V".to_vec(), Object::Int(2));
                    d.insert(b"R".to_vec(), Object::Int(3));
                }
                Encryption::Aes128 => {
                    d.insert(b"V".to_vec(), Object::Int(4));
                    d.insert(b"R".to_vec(), Object::Int(4));
                    let filter = dict(vec![
                        ("CFM", name("AESV2")),
                        ("Length", Object::Int(16)),
                        ("AuthEvent", name("DocOpen")),
                    ]);
                    d.insert(
                        b"CF".to_vec(),
                        Object::Dict(dict(vec![("StdCF", Object::Dict(filter))])),
                    );
                    d.insert(b"StmF".to_vec(), name("StdCF"));
                    d.insert(b"StrF".to_vec(), name("StdCF"));
                }
            }
            (d, key, e)
        });
        let encrypt_number = encryption
            .as_ref()
            .map(|(d, _, _)| self.add(Object::Dict(d.clone())));
        let crypt_for = |n: u32| -> Crypt<'_> {
            match (&encryption, encrypt_number) {
                (Some((_, key, e)), Some(en)) if en != n => Some((key.as_slice(), *e, n)),
                _ => None,
            }
        };
        let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let count = self.objects.len();
        let mut offsets: Vec<(u8, usize, usize)> = vec![(0, 0, 0); count];
        let packable =
            |o: &Object, n: u32| !matches!(o, Object::Stream(_)) && Some(n) != encrypt_number;
        let mut packed: Vec<u32> = Vec::new();
        if self.options.object_streams {
            for (n, o) in self.objects.iter().enumerate() {
                if let Some(o) = o
                    && n > 0
                    && packable(o, n as u32)
                {
                    packed.push(n as u32);
                }
            }
        }
        for (n, o) in self.objects.iter().enumerate() {
            let Some(o) = o else {
                continue;
            };
            if n == 0 || packed.contains(&(n as u32)) {
                continue;
            }
            offsets[n] = (1, out.len(), 0);
            let _ = writeln!(out, "{n} 0 obj");
            serialize(o, &mut out, crypt_for(n as u32));
            out.extend_from_slice(b"\nendobj\n");
        }
        let mut trailer = dict(vec![
            ("Root", reference(root)),
            (
                "ID",
                Object::Array(vec![
                    Object::String(id0.clone()),
                    Object::String(id0.clone()),
                ]),
            ),
        ]);
        if let Some(info) = info {
            trailer.insert(b"Info".to_vec(), reference(info));
        }
        if let Some(n) = encrypt_number {
            trailer.insert(b"Encrypt".to_vec(), reference(n));
        }
        if packed.is_empty() {
            let xref = out.len();
            let _ = write!(out, "xref\n0 {count}\n0000000000 65535 f \n");
            for entry in offsets.iter().skip(1) {
                if entry.0 == 1 {
                    let _ = writeln!(out, "{:010} 00000 n ", entry.1);
                } else {
                    out.extend_from_slice(b"0000000000 65535 f \n");
                }
            }
            trailer.insert(b"Size".to_vec(), Object::Int(count as i64));
            out.extend_from_slice(b"trailer\n");
            serialize_dict(&trailer, &mut out, None);
            let _ = write!(out, "\nstartxref\n{xref}\n%%EOF\n");
            return out;
        }
        // Object stream with the packed objects.
        let stream_number = count as u32;
        let mut header = String::new();
        let mut body = Vec::new();
        for (index, &n) in packed.iter().enumerate() {
            let _ = write!(header, "{n} {} ", body.len());
            if let Some(Some(o)) = self.objects.get(n as usize) {
                serialize(o, &mut body, None);
            }
            body.push(b'\n');
            offsets[n as usize] = (2, stream_number as usize, index);
        }
        let mut data = header.into_bytes();
        let first = data.len();
        data.extend_from_slice(&body);
        let mut d = dict(vec![
            ("Type", name("ObjStm")),
            ("N", Object::Int(packed.len() as i64)),
            ("First", Object::Int(first as i64)),
        ]);
        let data = if self.options.compress {
            d.insert(b"Filter".to_vec(), name("FlateDecode"));
            deflate(&data)
        } else {
            data
        };
        offsets.push((1, out.len(), 0));
        let _ = writeln!(out, "{stream_number} 0 obj");
        serialize(
            &Object::Stream(Stream {
                dict: d,
                data: Rc::new(data),
            }),
            &mut out,
            crypt_for(stream_number),
        );
        out.extend_from_slice(b"\nendobj\n");
        // Cross-reference stream: widths 1 4 2.
        let xref_number = stream_number + 1;
        let xref = out.len();
        offsets.push((1, xref, 0));
        let mut rows = Vec::new();
        for (i, entry) in offsets.iter().enumerate() {
            let (kind, a, b) = if i == 0 {
                (0u8, 0usize, 65_535usize)
            } else {
                *entry
            };
            rows.push(kind);
            rows.extend_from_slice(&(a as u32).to_be_bytes());
            rows.extend_from_slice(&(b as u16).to_be_bytes());
        }
        for (k, v) in [
            ("Type", name("XRef")),
            ("Size", Object::Int(offsets.len() as i64)),
            ("W", numbers(&[1.0, 4.0, 2.0])),
            ("Filter", name("FlateDecode")),
        ] {
            trailer.insert(k.as_bytes().to_vec(), v);
        }
        let _ = writeln!(out, "{xref_number} 0 obj");
        serialize(
            &Object::Stream(Stream {
                dict: trailer,
                data: Rc::new(deflate(&rows)),
            }),
            &mut out,
            None,
        );
        let _ = write!(out, "\nendobj\nstartxref\n{xref}\n%%EOF\n");
        out
    }
}
