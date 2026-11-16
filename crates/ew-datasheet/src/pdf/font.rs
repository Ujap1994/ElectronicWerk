//! Fonts: from the bytes of a text string to characters and their widths.
//! Simple fonts (Type 1, TrueType, Type 3) with their encodings and
//! `Differences`, composite fonts (Type 0) with `CMaps`, and `ToUnicode`.

use std::collections::HashMap;

use super::document::Document;
use super::encoding::{Base, glyph_name};
use super::metrics::Standard;
use super::object::{Dict, Lexer, Object, Token};

/// One character of a text string.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// Its text (usually one character; ligatures give more).
    pub text: String,
    /// Width in text space for a font size of 1.
    pub width: f64,
    /// A single-byte code 32 (word spacing applies).
    pub space: bool,
}

/// A `CMap`: code space and code mappings (to text or to CIDs).
#[derive(Debug, Clone, Default)]
pub struct CMap {
    /// Code space ranges: byte count, low and high bytes.
    ranges: Vec<(usize, Vec<u8>, Vec<u8>)>,
    /// Single codes to text.
    chars: HashMap<(usize, u32), String>,
    /// Code ranges to text: (bytes, low, high, first text as UTF-16).
    text_ranges: Vec<(usize, u32, u32, Vec<u16>)>,
    /// Code ranges with an array of texts.
    text_arrays: Vec<(usize, u32, Vec<String>)>,
    /// Code ranges to CIDs: (bytes, low, high, first CID).
    cid_ranges: Vec<(usize, u32, u32, u32)>,
}

fn value(bytes: &[u8]) -> u32 {
    bytes.iter().take(4).fold(0, |v, &b| v << 8 | u32::from(b))
}

fn utf16(bytes: &[u8]) -> Vec<u16> {
    if bytes.len() == 1 {
        return vec![u16::from(bytes[0])];
    }
    bytes
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
        .collect()
}

impl CMap {
    /// Parses a `CMap` stream (`ToUnicode` or encoding).
    #[must_use]
    pub fn parse(data: &[u8]) -> Self {
        let mut map = Self::default();
        let mut lexer = Lexer::new(data, 0);
        let mut operands: Vec<Object> = Vec::new();
        while let Some(token) = lexer.token() {
            let Token::Keyword(word) = token else {
                if let Some(o) = lexer.from_token(token, false, 0) {
                    operands.push(o);
                }
                continue;
            };
            match word {
                b"endcodespacerange" => {
                    for pair in operands.chunks_exact(2) {
                        if let (Some(lo), Some(hi)) = (pair[0].string(), pair[1].string())
                            && !lo.is_empty()
                            && lo.len() == hi.len()
                        {
                            map.ranges.push((lo.len(), lo.to_vec(), hi.to_vec()));
                        }
                    }
                }
                b"endbfchar" => {
                    for pair in operands.chunks_exact(2) {
                        let Some(src) = pair[0].string() else {
                            continue;
                        };
                        let text = match &pair[1] {
                            Object::String(dst) => String::from_utf16_lossy(&utf16(dst)),
                            Object::Name(n) => {
                                glyph_name(&String::from_utf8_lossy(n)).unwrap_or_default()
                            }
                            _ => continue,
                        };
                        map.chars.insert((src.len(), value(src)), text);
                    }
                }
                b"endbfrange" => {
                    for triple in operands.chunks_exact(3) {
                        let (Some(lo), Some(hi)) = (triple[0].string(), triple[1].string()) else {
                            continue;
                        };
                        let (lo_v, hi_v) = (value(lo), value(hi));
                        if hi_v < lo_v {
                            continue;
                        }
                        match &triple[2] {
                            Object::String(dst) => {
                                map.text_ranges.push((lo.len(), lo_v, hi_v, utf16(dst)));
                            }
                            Object::Array(items) => {
                                let texts = items
                                    .iter()
                                    .map(|o| {
                                        o.string()
                                            .map(|s| String::from_utf16_lossy(&utf16(s)))
                                            .unwrap_or_default()
                                    })
                                    .collect();
                                map.text_arrays.push((lo.len(), lo_v, texts));
                            }
                            _ => {}
                        }
                    }
                }
                b"endcidrange" => {
                    for triple in operands.chunks_exact(3) {
                        if let (Some(lo), Some(hi), Some(cid)) =
                            (triple[0].string(), triple[1].string(), triple[2].int())
                        {
                            map.cid_ranges.push((
                                lo.len(),
                                value(lo),
                                value(hi),
                                cid.max(0) as u32,
                            ));
                        }
                    }
                }
                b"endcidchar" => {
                    for pair in operands.chunks_exact(2) {
                        if let (Some(code), Some(cid)) = (pair[0].string(), pair[1].int()) {
                            let v = value(code);
                            map.cid_ranges.push((code.len(), v, v, cid.max(0) as u32));
                        }
                    }
                }
                _ => {}
            }
            operands.clear();
        }
        map
    }

    /// The text of a code, if mapped.
    #[must_use]
    pub fn text(&self, bytes: usize, code: u32) -> Option<String> {
        if let Some(t) = self.chars.get(&(bytes, code)) {
            return Some(t.clone());
        }
        for (n, lo, hi, first) in &self.text_ranges {
            if *n == bytes && (*lo..=*hi).contains(&code) {
                let mut units = first.clone();
                if let Some(last) = units.last_mut() {
                    *last = last.wrapping_add((code - lo) as u16);
                }
                return Some(String::from_utf16_lossy(&units));
            }
        }
        for (n, lo, texts) in &self.text_arrays {
            if *n == bytes && code >= *lo {
                if let Some(t) = texts.get((code - lo) as usize) {
                    return Some(t.clone());
                }
            }
        }
        None
    }

    /// The CID of a code (identity when unmapped).
    #[must_use]
    pub fn cid(&self, bytes: usize, code: u32) -> u32 {
        self.cid_ranges
            .iter()
            .find(|(n, lo, hi, _)| *n == bytes && (*lo..=*hi).contains(&code))
            .map_or(code, |(_, lo, _, cid)| cid + (code - lo))
    }

    /// The byte count of the code at the start of `bytes`.
    #[must_use]
    pub fn code_length(&self, bytes: &[u8]) -> usize {
        for (n, lo, hi) in &self.ranges {
            if bytes.len() >= *n && (0..*n).all(|i| lo[i] <= bytes[i] && bytes[i] <= hi[i]) {
                return *n;
            }
        }
        // Not in any range: the shortest code length.
        self.ranges.iter().map(|(n, _, _)| *n).min().unwrap_or(1)
    }

    fn has_ranges(&self) -> bool {
        !self.ranges.is_empty()
    }
}

#[derive(Debug, Clone)]
enum Kind {
    /// One byte per code; text by code.
    Simple { encoding: Vec<Option<String>> },
    /// Codes by a `CMap`; `unicode` when the codes are UCS-2.
    Composite { cmap: CMap, unicode: bool },
}

/// A font ready to decode strings.
#[derive(Debug, Clone)]
pub struct Font {
    /// Font name without the subset tag.
    pub name: String,
    /// Bold weight.
    pub bold: bool,
    kind: Kind,
    to_unicode: Option<CMap>,
    widths: HashMap<u32, f64>,
    default_width: f64,
    standard: Option<Standard>,
    /// Glyph space to text space (0.001, or a Type 3 font matrix).
    scale: f64,
}

impl Font {
    /// A font for text that names no usable font.
    #[must_use]
    pub fn fallback() -> Self {
        Self {
            name: "Helvetica".into(),
            bold: false,
            kind: Kind::Simple {
                encoding: (0..=255u8)
                    .map(|c| Base::WinAnsi.char(c).map(String::from))
                    .collect(),
            },
            to_unicode: None,
            widths: HashMap::new(),
            default_width: 556.0,
            standard: Some(Standard::Helvetica),
            scale: 0.001,
        }
    }

    /// Reads a font dictionary.
    #[must_use]
    pub fn load(doc: &Document, dict: &Dict) -> Self {
        let subtype = doc.entry(dict, "Subtype");
        let base_font = doc
            .entry(dict, "BaseFont")
            .name()
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        let name = base_font
            .split_once('+')
            .filter(|(tag, _)| tag.len() == 6)
            .map_or(base_font.as_str(), |(_, rest)| rest)
            .to_owned();
        let to_unicode = match doc.entry(dict, "ToUnicode") {
            Object::Stream(s) => Some(CMap::parse(&doc.decode(&s))),
            _ => None,
        };
        let (descriptor_dict, kind, widths, default_width, scale) =
            if subtype.name() == Some(b"Type0") {
                Self::composite(doc, dict)
            } else {
                Self::simple(doc, dict, &name, subtype.name() == Some(b"Type3"))
            };
        let flags = descriptor_dict
            .as_ref()
            .map_or(0, |d| doc.entry(d, "Flags").int().unwrap_or(0));
        let weight = descriptor_dict
            .as_ref()
            .and_then(|d| doc.entry(d, "FontWeight").number())
            .unwrap_or(0.0);
        let lower = name.to_ascii_lowercase();
        let bold = weight >= 600.0
            || flags & 0x40000 != 0
            || ["bold", "black", "heavy", "semibold", "demi"]
                .iter()
                .any(|w| lower.contains(w));
        let standard = widths.is_empty().then(|| Standard::for_name(&name));
        Self {
            name,
            bold,
            kind,
            to_unicode,
            widths,
            default_width,
            standard,
            scale,
        }
    }

    #[allow(
        clippy::type_complexity,
        reason = "the parts of a font, built together"
    )]
    fn simple(
        doc: &Document,
        dict: &Dict,
        name: &str,
        type3: bool,
    ) -> (Option<Dict>, Kind, HashMap<u32, f64>, f64, f64) {
        let descriptor = doc.entry(dict, "FontDescriptor").dict().cloned();
        let flags = descriptor
            .as_ref()
            .map_or(0, |d| doc.entry(d, "Flags").int().unwrap_or(0));
        let symbolic = flags & 4 != 0;
        let lower = name.to_ascii_lowercase();
        let subtype = doc.entry(dict, "Subtype");
        let truetype = subtype.name() == Some(b"TrueType");
        let encoding = doc.entry(dict, "Encoding");
        let (named, differences) = match &encoding {
            Object::Name(n) => (Base::from_name(n), None),
            Object::Dict(d) => (
                doc.entry(d, "BaseEncoding")
                    .name()
                    .and_then(Base::from_name),
                Some(doc.entry(d, "Differences")),
            ),
            _ => (None, None),
        };
        let builtin = if named.is_none() {
            descriptor.as_ref().and_then(|d| builtin_encoding(doc, d))
        } else {
            None
        };
        let base = named.unwrap_or(if lower.contains("symbol") {
            Base::Symbol
        } else if lower.contains("dingbats") {
            Base::Dingbats
        } else if symbolic && truetype {
            Base::Latin1
        } else if truetype {
            Base::WinAnsi
        } else {
            Base::Standard
        });
        let mut table: Vec<Option<String>> = (0..=255u8)
            .map(|c| {
                builtin
                    .as_ref()
                    .and_then(|b| b.get(&c).cloned())
                    .or_else(|| base.char(c).map(String::from))
            })
            .collect();
        if let Some(Object::Array(items)) = differences {
            let mut code = 0usize;
            for item in items {
                match item {
                    Object::Int(c) => code = c.clamp(0, 255) as usize,
                    Object::Name(n) => {
                        if code < 256 {
                            let text = glyph_name(&String::from_utf8_lossy(&n));
                            if text.is_some() || table[code].is_none() {
                                table[code] = text;
                            }
                        }
                        code += 1;
                    }
                    _ => {}
                }
            }
        }
        let first = doc.entry(dict, "FirstChar").int().unwrap_or(0).max(0) as u32;
        let mut widths = HashMap::new();
        if let Some(list) = doc.entry(dict, "Widths").array() {
            for (i, w) in list.iter().enumerate() {
                widths.insert(first + i as u32, doc.resolve(w).number().unwrap_or(0.0));
            }
        }
        let missing = descriptor
            .as_ref()
            .and_then(|d| doc.entry(d, "MissingWidth").number())
            .unwrap_or(0.0);
        let scale = if type3 {
            doc.entry(dict, "FontMatrix")
                .numbers()
                .first()
                .copied()
                .filter(|v| *v != 0.0)
                .unwrap_or(0.001)
        } else {
            0.001
        };
        (
            descriptor,
            Kind::Simple { encoding: table },
            widths,
            missing,
            scale,
        )
    }

    #[allow(
        clippy::type_complexity,
        reason = "the parts of a font, built together"
    )]
    fn composite(doc: &Document, dict: &Dict) -> (Option<Dict>, Kind, HashMap<u32, f64>, f64, f64) {
        let encoding = doc.entry(dict, "Encoding");
        let (cmap, unicode) = match &encoding {
            Object::Stream(s) => (CMap::parse(&doc.decode(s)), false),
            Object::Name(n) => {
                let name = String::from_utf8_lossy(n);
                let mut cmap = CMap::default();
                if name.starts_with("Identity") || name.contains("UCS2") || name.contains("UTF16") {
                    cmap.ranges.push((2, vec![0, 0], vec![0xff, 0xff]));
                } else {
                    // Other predefined CMaps: one byte below 0x80, else two.
                    cmap.ranges.push((1, vec![0], vec![0x7f]));
                    cmap.ranges.push((2, vec![0x80, 0], vec![0xff, 0xff]));
                }
                (cmap, name.contains("UCS2") || name.contains("UTF16"))
            }
            _ => {
                let mut cmap = CMap::default();
                cmap.ranges.push((2, vec![0, 0], vec![0xff, 0xff]));
                (cmap, false)
            }
        };
        let cmap = if cmap.has_ranges() {
            cmap
        } else {
            let mut c = cmap;
            c.ranges.push((2, vec![0, 0], vec![0xff, 0xff]));
            c
        };
        let descendant = doc
            .entry(dict, "DescendantFonts")
            .array()
            .and_then(|a| a.first())
            .map(|o| doc.resolve(o));
        let mut widths = HashMap::new();
        let mut default_width = 1000.0;
        let mut descriptor = None;
        if let Some(d) = descendant.as_ref().and_then(Object::dict) {
            default_width = doc.entry(d, "DW").number().unwrap_or(1000.0);
            descriptor = doc.entry(d, "FontDescriptor").dict().cloned();
            let list = doc.entry(d, "W");
            let items: Vec<Object> = list
                .array()
                .unwrap_or_default()
                .iter()
                .map(|o| doc.resolve(o))
                .collect();
            let mut i = 0;
            while i < items.len() {
                let Some(first) = items[i].int() else {
                    i += 1;
                    continue;
                };
                match items.get(i + 1) {
                    Some(Object::Array(ws)) => {
                        for (k, w) in ws.iter().enumerate() {
                            widths.insert(
                                first.max(0) as u32 + k as u32,
                                doc.resolve(w).number().unwrap_or(default_width),
                            );
                        }
                        i += 2;
                    }
                    Some(end) => {
                        let (Some(end), Some(w)) =
                            (end.int(), items.get(i + 2).and_then(Object::number))
                        else {
                            break;
                        };
                        for cid in first.max(0)..=end.min(first + 65_535) {
                            widths.insert(cid as u32, w);
                        }
                        i += 3;
                    }
                    None => break,
                }
            }
        }
        (
            descriptor,
            Kind::Composite { cmap, unicode },
            widths,
            default_width,
            0.001,
        )
    }

    /// Decodes a string of a text operator.
    #[must_use]
    pub fn decode(&self, bytes: &[u8]) -> Vec<Glyph> {
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let (n, code, cid, fallback) = match &self.kind {
                Kind::Simple { encoding } => {
                    let c = bytes[i];
                    (
                        1,
                        u32::from(c),
                        u32::from(c),
                        encoding[usize::from(c)].clone(),
                    )
                }
                Kind::Composite { cmap, unicode } => {
                    let n = cmap
                        .code_length(&bytes[i..])
                        .clamp(1, 4)
                        .min(bytes.len() - i);
                    let code = value(&bytes[i..i + n]);
                    let text = if *unicode {
                        char::from_u32(code).map(String::from)
                    } else {
                        None
                    };
                    (n, code, cmap.cid(n, code), text)
                }
            };
            let text = self
                .to_unicode
                .as_ref()
                .and_then(|m| m.text(n, code))
                .or(fallback)
                .unwrap_or_else(|| "\u{fffd}".to_owned());
            let width = self.widths.get(&cid).copied().unwrap_or_else(|| {
                match (&self.kind, self.standard, text.chars().next()) {
                    (Kind::Simple { .. }, Some(s), Some(c)) => s.width(c),
                    (Kind::Simple { .. }, None, Some(c)) if self.default_width == 0.0 => {
                        Standard::Helvetica.width(c)
                    }
                    _ => self.default_width,
                }
            });
            out.push(Glyph {
                space: n == 1 && code == 32,
                text,
                width: width * self.scale,
            });
            i += n;
        }
        out
    }
}

/// The built-in encoding of an embedded Type 1 font (`dup 32 /space put`
/// in the clear-text part of the font program).
fn builtin_encoding(doc: &Document, descriptor: &Dict) -> Option<HashMap<u8, String>> {
    let Object::Stream(stream) = doc.entry(descriptor, "FontFile") else {
        return None;
    };
    let data = doc.decode(&stream);
    let clear = doc
        .entry(&stream.dict, "Length1")
        .int()
        .map_or(data.len(), |n| (n.max(0) as usize).min(data.len()));
    let text = &data[..clear];
    let start = text.windows(9).position(|w| w == b"/Encoding")?;
    let rest = &text[start..];
    if rest.starts_with(b"/Encoding StandardEncoding") {
        return None;
    }
    let mut out = HashMap::new();
    let mut lexer = Lexer::new(rest, 0);
    let mut last: Vec<Token<'_>> = Vec::new();
    while let Some(token) = lexer.token() {
        if token == Token::Keyword(b"readonly") || token == Token::Keyword(b"def") {
            break;
        }
        if token == Token::Keyword(b"put")
            && let [Token::Keyword(b"dup"), Token::Int(code), Token::Name(name)] = last.as_slice()
            && let (Ok(code), Some(text)) = (
                u8::try_from(*code),
                glyph_name(&String::from_utf8_lossy(name)),
            )
        {
            out.insert(code, text);
        }
        last.push(token);
        if last.len() > 3 {
            last.remove(0);
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_to_unicode_cmaps() {
        let data = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap
            1 begincodespacerange <0000> <FFFF> endcodespacerange
            2 beginbfchar <0003> <0020> <0011> <00660069> endbfchar
            2 beginbfrange <0024> <0026> <0041> <0030> <0031> [<00B5> <03A9>] endbfrange
            endcmap CMapName currentdict /CMap defineresource pop end end";
        let cmap = CMap::parse(data);
        assert_eq!(cmap.text(2, 3).as_deref(), Some(" "));
        assert_eq!(cmap.text(2, 0x11).as_deref(), Some("fi"));
        assert_eq!(cmap.text(2, 0x25).as_deref(), Some("B"));
        assert_eq!(cmap.text(2, 0x31).as_deref(), Some("Ω"));
        assert_eq!(cmap.text(2, 0x99), None);
        assert_eq!(cmap.code_length(&[0, 3]), 2);
    }

    #[test]
    fn parses_cid_ranges() {
        let data = b"1 begincodespacerange <00> <FF> endcodespacerange
            1 begincidrange <20> <7E> 1 endcidrange";
        let cmap = CMap::parse(data);
        assert_eq!(cmap.cid(1, 0x41), 34);
        assert_eq!(cmap.code_length(b"A"), 1);
    }
}
