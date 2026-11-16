//! Character encodings of simple fonts and glyph names, as far as
//! datasheets need them: the standard Latin encodings, the Symbol font
//! (Greek letters and the signs of units and tolerances) and the glyph
//! names of the Adobe glyph list for these characters.

/// A base encoding of a simple font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    /// Adobe standard encoding.
    Standard,
    /// Windows code page 1252.
    WinAnsi,
    /// Mac OS Roman.
    MacRoman,
    /// The Symbol font's own encoding.
    Symbol,
    /// The `ZapfDingbats` font (only the space is known).
    Dingbats,
    /// Codes are characters (symbolic TrueType fonts, Latin-1).
    Latin1,
}

impl Base {
    /// The encoding of a `/Encoding` name.
    #[must_use]
    pub fn from_name(name: &[u8]) -> Option<Self> {
        match name {
            b"StandardEncoding" => Some(Self::Standard),
            b"WinAnsiEncoding" => Some(Self::WinAnsi),
            b"MacRomanEncoding" | b"MacExpertEncoding" => Some(Self::MacRoman),
            _ => None,
        }
    }

    /// The character of a code.
    #[must_use]
    pub fn char(self, code: u8) -> Option<char> {
        match self {
            Self::Standard => standard(code),
            Self::WinAnsi => win_ansi(code),
            Self::MacRoman => mac_roman(code),
            Self::Symbol => symbol(code),
            Self::Dingbats => (code == 0x20).then_some(' '),
            Self::Latin1 => Some(char::from(code)),
        }
    }
}

const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

/// Windows code page 1252 (`WinAnsiEncoding`).
#[must_use]
pub fn win_ansi(code: u8) -> Option<char> {
    match code {
        0x20..=0x7e | 0xa0..=0xff => Some(char::from(code)),
        0x80..=0x9f => {
            let u = CP1252_HIGH[usize::from(code - 0x80)];
            if u == 0 {
                None
            } else {
                char::from_u32(u32::from(u))
            }
        }
        0x7f => Some('\u{2022}'),
        _ => None,
    }
}

const MAC_HIGH: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";

/// Mac OS Roman.
#[must_use]
pub fn mac_roman(code: u8) -> Option<char> {
    match code {
        0x20..=0x7e => Some(char::from(code)),
        0x80..=0xff => MAC_HIGH.chars().nth(usize::from(code - 0x80)),
        _ => None,
    }
}

const STANDARD_HIGH: [(u8, char); 61] = [
    (0xa1, '¡'),
    (0xa2, '¢'),
    (0xa3, '£'),
    (0xa4, '⁄'),
    (0xa5, '¥'),
    (0xa6, 'ƒ'),
    (0xa7, '§'),
    (0xa8, '¤'),
    (0xa9, '\''),
    (0xaa, '“'),
    (0xab, '«'),
    (0xac, '‹'),
    (0xad, '›'),
    (0xae, 'ﬁ'),
    (0xaf, 'ﬂ'),
    (0xb1, '–'),
    (0xb2, '†'),
    (0xb3, '‡'),
    (0xb4, '·'),
    (0xb6, '¶'),
    (0xb7, '•'),
    (0xb8, '‚'),
    (0xb9, '„'),
    (0xba, '”'),
    (0xbb, '»'),
    (0xbc, '…'),
    (0xbd, '‰'),
    (0xbf, '¿'),
    (0xc1, '`'),
    (0xc2, '´'),
    (0xc3, 'ˆ'),
    (0xc4, '˜'),
    (0xc5, '¯'),
    (0xc6, '˘'),
    (0xc7, '˙'),
    (0xc8, '¨'),
    (0xca, '˚'),
    (0xcb, '¸'),
    (0xcd, '˝'),
    (0xce, '˛'),
    (0xcf, 'ˇ'),
    (0xd0, '—'),
    (0xe1, 'Æ'),
    (0xe3, 'ª'),
    (0xe8, 'Ł'),
    (0xe9, 'Ø'),
    (0xea, 'Œ'),
    (0xeb, 'º'),
    (0xf1, 'æ'),
    (0xf5, 'ı'),
    (0xf8, 'ł'),
    (0xf9, 'ø'),
    (0xfa, 'œ'),
    (0xfb, 'ß'),
    (0x27, '’'),
    (0x60, '‘'),
    (0xb0, '\u{fffd}'),
    (0xb5, '\u{fffd}'),
    (0xbe, '\u{fffd}'),
    (0xc0, '\u{fffd}'),
    (0xc9, '\u{fffd}'),
];

/// Adobe standard encoding.
#[must_use]
pub fn standard(code: u8) -> Option<char> {
    if let Some((_, c)) = STANDARD_HIGH.iter().find(|(k, _)| *k == code) {
        return (*c != '\u{fffd}').then_some(*c);
    }
    (0x20..=0x7e).contains(&code).then(|| char::from(code))
}

const SYMBOL_LOW: &str = " !∀#∃%&∋()∗+,−./0123456789:;<=>?≅ΑΒΧΔΕΦΓΗΙϑΚΛΜΝΟΠΘΡΣΤΥςΩΞΨΖ[∴]⊥_‾αβχδεφγηιϕκλμνοπθρστυϖωξψζ{|}∼";

const SYMBOL_HIGH: [(u8, char); 70] = [
    (0xa0, '€'),
    (0xa1, 'ϒ'),
    (0xa2, '′'),
    (0xa3, '≤'),
    (0xa4, '⁄'),
    (0xa5, '∞'),
    (0xa6, 'ƒ'),
    (0xa7, '♣'),
    (0xa8, '♦'),
    (0xa9, '♥'),
    (0xaa, '♠'),
    (0xab, '↔'),
    (0xac, '←'),
    (0xad, '↑'),
    (0xae, '→'),
    (0xaf, '↓'),
    (0xb0, '°'),
    (0xb1, '±'),
    (0xb2, '″'),
    (0xb3, '≥'),
    (0xb4, '×'),
    (0xb5, '∝'),
    (0xb6, '∂'),
    (0xb7, '•'),
    (0xb8, '÷'),
    (0xb9, '≠'),
    (0xba, '≡'),
    (0xbb, '≈'),
    (0xbc, '…'),
    (0xbd, '⏐'),
    (0xbe, '⎯'),
    (0xbf, '↵'),
    (0xc0, 'ℵ'),
    (0xc1, 'ℑ'),
    (0xc2, 'ℜ'),
    (0xc3, '℘'),
    (0xc4, '⊗'),
    (0xc5, '⊕'),
    (0xc6, '∅'),
    (0xc7, '∩'),
    (0xc8, '∪'),
    (0xc9, '⊃'),
    (0xca, '⊇'),
    (0xcb, '⊄'),
    (0xcc, '⊂'),
    (0xcd, '⊆'),
    (0xce, '∈'),
    (0xcf, '∉'),
    (0xd0, '∠'),
    (0xd1, '∇'),
    (0xd2, '®'),
    (0xd3, '©'),
    (0xd4, '™'),
    (0xd5, '∏'),
    (0xd6, '√'),
    (0xd7, '⋅'),
    (0xd8, '¬'),
    (0xd9, '∧'),
    (0xda, '∨'),
    (0xdb, '⇔'),
    (0xdc, '⇐'),
    (0xdd, '⇑'),
    (0xde, '⇒'),
    (0xdf, '⇓'),
    (0xe0, '◊'),
    (0xe1, '〈'),
    (0xe2, '®'),
    (0xe5, '∑'),
    (0xf1, '〉'),
    (0xf2, '∫'),
];

/// The Symbol font's encoding.
#[must_use]
pub fn symbol(code: u8) -> Option<char> {
    match code {
        0x20..=0x7e => SYMBOL_LOW.chars().nth(usize::from(code - 0x20)),
        _ => SYMBOL_HIGH
            .iter()
            .find(|(k, _)| *k == code)
            .map(|(_, c)| *c),
    }
}

/// `PDFDocEncoding` of text strings (document information, bookmarks).
#[must_use]
pub fn pdf_doc(code: u8) -> char {
    const SPECIAL: [(u8, char); 39] = [
        (0x18, '˘'),
        (0x19, 'ˇ'),
        (0x1a, 'ˆ'),
        (0x1b, '˙'),
        (0x1c, '˝'),
        (0x1d, '˛'),
        (0x1e, '˚'),
        (0x1f, '˜'),
        (0x80, '•'),
        (0x81, '†'),
        (0x82, '‡'),
        (0x83, '…'),
        (0x84, '—'),
        (0x85, '–'),
        (0x86, 'ƒ'),
        (0x87, '⁄'),
        (0x88, '‹'),
        (0x89, '›'),
        (0x8a, '−'),
        (0x8b, '‰'),
        (0x8c, '„'),
        (0x8d, '“'),
        (0x8e, '”'),
        (0x8f, '‘'),
        (0x90, '’'),
        (0x91, '‚'),
        (0x92, '™'),
        (0x93, 'ﬁ'),
        (0x94, 'ﬂ'),
        (0x95, 'Ł'),
        (0x96, 'Œ'),
        (0x97, 'Š'),
        (0x98, 'Ÿ'),
        (0x99, 'Ž'),
        (0x9a, 'ı'),
        (0x9b, 'ł'),
        (0x9c, 'œ'),
        (0x9d, 'š'),
        (0x9e, 'ž'),
    ];
    SPECIAL
        .iter()
        .find(|(k, _)| *k == code)
        .map_or_else(|| char::from(code), |(_, c)| *c)
}

/// A text string: UTF-16 with a byte order mark, UTF-8 with one (PDF 2.0),
/// or `PDFDocEncoding`.
#[must_use]
pub fn text_string(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    bytes.iter().map(|&b| pdf_doc(b)).collect()
}

/// Glyph names of the Adobe glyph list that datasheets use (Latin, Greek,
/// signs of units and mathematics).
const NAMES: &[(&str, &str)] = &[
    ("space", " "),
    ("nbspace", " "),
    ("exclam", "!"),
    ("quotedbl", "\""),
    ("numbersign", "#"),
    ("dollar", "$"),
    ("percent", "%"),
    ("ampersand", "&"),
    ("quotesingle", "'"),
    ("quoteright", "’"),
    ("parenleft", "("),
    ("parenright", ")"),
    ("asterisk", "*"),
    ("plus", "+"),
    ("comma", ","),
    ("hyphen", "-"),
    ("sfthyphen", "-"),
    ("period", "."),
    ("slash", "/"),
    ("zero", "0"),
    ("one", "1"),
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("colon", ":"),
    ("semicolon", ";"),
    ("less", "<"),
    ("equal", "="),
    ("greater", ">"),
    ("question", "?"),
    ("at", "@"),
    ("bracketleft", "["),
    ("backslash", "\\"),
    ("bracketright", "]"),
    ("asciicircum", "^"),
    ("underscore", "_"),
    ("grave", "`"),
    ("quoteleft", "‘"),
    ("braceleft", "{"),
    ("bar", "|"),
    ("braceright", "}"),
    ("asciitilde", "~"),
    ("exclamdown", "¡"),
    ("cent", "¢"),
    ("sterling", "£"),
    ("currency", "¤"),
    ("yen", "¥"),
    ("brokenbar", "¦"),
    ("section", "§"),
    ("dieresis", "¨"),
    ("copyright", "©"),
    ("ordfeminine", "ª"),
    ("guillemotleft", "«"),
    ("logicalnot", "¬"),
    ("registered", "®"),
    ("macron", "¯"),
    ("degree", "°"),
    ("plusminus", "±"),
    ("twosuperior", "²"),
    ("threesuperior", "³"),
    ("acute", "´"),
    ("mu", "µ"),
    ("micro", "µ"),
    ("paragraph", "¶"),
    ("periodcentered", "·"),
    ("cedilla", "¸"),
    ("onesuperior", "¹"),
    ("ordmasculine", "º"),
    ("guillemotright", "»"),
    ("onequarter", "¼"),
    ("onehalf", "½"),
    ("threequarters", "¾"),
    ("questiondown", "¿"),
    ("Agrave", "À"),
    ("Aacute", "Á"),
    ("Acircumflex", "Â"),
    ("Atilde", "Ã"),
    ("Adieresis", "Ä"),
    ("Aring", "Å"),
    ("AE", "Æ"),
    ("Ccedilla", "Ç"),
    ("Egrave", "È"),
    ("Eacute", "É"),
    ("Ecircumflex", "Ê"),
    ("Edieresis", "Ë"),
    ("Igrave", "Ì"),
    ("Iacute", "Í"),
    ("Icircumflex", "Î"),
    ("Idieresis", "Ï"),
    ("Eth", "Ð"),
    ("Ntilde", "Ñ"),
    ("Ograve", "Ò"),
    ("Oacute", "Ó"),
    ("Ocircumflex", "Ô"),
    ("Otilde", "Õ"),
    ("Odieresis", "Ö"),
    ("multiply", "×"),
    ("Oslash", "Ø"),
    ("Ugrave", "Ù"),
    ("Uacute", "Ú"),
    ("Ucircumflex", "Û"),
    ("Udieresis", "Ü"),
    ("Yacute", "Ý"),
    ("Thorn", "Þ"),
    ("germandbls", "ß"),
    ("agrave", "à"),
    ("aacute", "á"),
    ("acircumflex", "â"),
    ("atilde", "ã"),
    ("adieresis", "ä"),
    ("aring", "å"),
    ("ae", "æ"),
    ("ccedilla", "ç"),
    ("egrave", "è"),
    ("eacute", "é"),
    ("ecircumflex", "ê"),
    ("edieresis", "ë"),
    ("igrave", "ì"),
    ("iacute", "í"),
    ("icircumflex", "î"),
    ("idieresis", "ï"),
    ("eth", "ð"),
    ("ntilde", "ñ"),
    ("ograve", "ò"),
    ("oacute", "ó"),
    ("ocircumflex", "ô"),
    ("otilde", "õ"),
    ("odieresis", "ö"),
    ("divide", "÷"),
    ("oslash", "ø"),
    ("ugrave", "ù"),
    ("uacute", "ú"),
    ("ucircumflex", "û"),
    ("udieresis", "ü"),
    ("yacute", "ý"),
    ("thorn", "þ"),
    ("ydieresis", "ÿ"),
    ("Euro", "€"),
    ("quotesinglbase", "‚"),
    ("florin", "ƒ"),
    ("quotedblbase", "„"),
    ("ellipsis", "…"),
    ("dagger", "†"),
    ("daggerdbl", "‡"),
    ("circumflex", "ˆ"),
    ("perthousand", "‰"),
    ("Scaron", "Š"),
    ("guilsinglleft", "‹"),
    ("OE", "Œ"),
    ("Zcaron", "Ž"),
    ("quotedblleft", "“"),
    ("quotedblright", "”"),
    ("bullet", "•"),
    ("endash", "–"),
    ("emdash", "—"),
    ("tilde", "˜"),
    ("trademark", "™"),
    ("scaron", "š"),
    ("guilsinglright", "›"),
    ("oe", "œ"),
    ("zcaron", "ž"),
    ("Ydieresis", "Ÿ"),
    ("fi", "fi"),
    ("fl", "fl"),
    ("ff", "ff"),
    ("ffi", "ffi"),
    ("ffl", "ffl"),
    ("fraction", "⁄"),
    ("Lslash", "Ł"),
    ("lslash", "ł"),
    ("dotlessi", "ı"),
    ("ring", "˚"),
    ("breve", "˘"),
    ("dotaccent", "˙"),
    ("hungarumlaut", "˝"),
    ("ogonek", "˛"),
    ("caron", "ˇ"),
    ("minus", "−"),
    ("Alpha", "Α"),
    ("Beta", "Β"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Epsilon", "Ε"),
    ("Zeta", "Ζ"),
    ("Eta", "Η"),
    ("Theta", "Θ"),
    ("Iota", "Ι"),
    ("Kappa", "Κ"),
    ("Lambda", "Λ"),
    ("Mu", "Μ"),
    ("Nu", "Ν"),
    ("Xi", "Ξ"),
    ("Omicron", "Ο"),
    ("Pi", "Π"),
    ("Rho", "Ρ"),
    ("Sigma", "Σ"),
    ("Tau", "Τ"),
    ("Upsilon", "Υ"),
    ("Phi", "Φ"),
    ("Chi", "Χ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("theta1", "ϑ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("omicron", "ο"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("sigma1", "ς"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "φ"),
    ("phi1", "ϕ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("lessequal", "≤"),
    ("greaterequal", "≥"),
    ("notequal", "≠"),
    ("approxequal", "≈"),
    ("infinity", "∞"),
    ("partialdiff", "∂"),
    ("summation", "∑"),
    ("product", "∏"),
    ("radical", "√"),
    ("integral", "∫"),
    ("similar", "∼"),
    ("congruent", "≅"),
    ("equivalence", "≡"),
    ("proportional", "∝"),
    ("element", "∈"),
    ("notelement", "∉"),
    ("arrowright", "→"),
    ("arrowleft", "←"),
    ("arrowup", "↑"),
    ("arrowdown", "↓"),
    ("arrowboth", "↔"),
    ("arrowdblright", "⇒"),
    ("arrowdblleft", "⇐"),
    ("lozenge", "◊"),
    ("angle", "∠"),
    ("circlemultiply", "⊗"),
    ("circleplus", "⊕"),
    ("emptyset", "∅"),
    ("intersection", "∩"),
    ("union", "∪"),
    ("gradient", "∇"),
    ("logicaland", "∧"),
    ("logicalor", "∨"),
    ("dotmath", "⋅"),
    ("prime", "′"),
    ("second", "″"),
    ("Ohm", "Ω"),
    ("asteriskmath", "∗"),
    ("universal", "∀"),
    ("existential", "∃"),
    ("suchthat", "∋"),
    ("therefore", "∴"),
    ("perpendicular", "⊥"),
    ("radicalex", "‾"),
    ("aleph", "ℵ"),
    ("Upsilon1", "ϒ"),
    ("omega1", "ϖ"),
    ("club", "♣"),
    ("diamond", "♦"),
    ("heart", "♥"),
    ("spade", "♠"),
    ("carriagereturn", "↵"),
    ("registersans", "®"),
    ("copyrightsans", "©"),
    ("trademarksans", "™"),
    ("angleleft", "〈"),
    ("angleright", "〉"),
];

/// The text of a glyph name: the Adobe glyph list, `uniXXXX…`, `uXXXX`,
/// suffixes (`A.sc`) and ligatures (`f_f_i`).
#[must_use]
pub fn glyph_name(name: &str) -> Option<String> {
    if let Some((_, text)) = NAMES.iter().find(|(n, _)| *n == name) {
        return Some((*text).to_owned());
    }
    let base = name.split('.').next().unwrap_or(name);
    if base.len() == 1 && base.as_bytes()[0].is_ascii_alphabetic() {
        return Some(base.to_owned());
    }
    if base != name && !base.is_empty() {
        return glyph_name(base);
    }
    if base.contains('_') {
        let parts: Option<Vec<String>> = base.split('_').map(glyph_name).collect();
        return parts.map(|p| p.concat());
    }
    if let Some(hex) = base.strip_prefix("uni")
        && hex.len() >= 4
        && hex.len() % 4 == 0
    {
        let units: Option<Vec<u16>> = (0..hex.len() / 4)
            .map(|i| u16::from_str_radix(&hex[i * 4..i * 4 + 4], 16).ok())
            .collect();
        return units.map(|u| String::from_utf16_lossy(&u));
    }
    if let Some(hex) = base.strip_prefix('u')
        && (4..=6).contains(&hex.len())
        && let Ok(v) = u32::from_str_radix(hex, 16)
    {
        return char::from_u32(v).map(String::from);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_latin_encodings() {
        assert_eq!(win_ansi(0x80), Some('€'));
        assert_eq!(win_ansi(0xb5), Some('µ'));
        assert_eq!(win_ansi(0x96), Some('–'));
        assert_eq!(mac_roman(0xb5), Some('µ'));
        assert_eq!(mac_roman(0xbd), Some('Ω'));
        assert_eq!(mac_roman(0xff), Some('ˇ'));
        assert_eq!(standard(0x27), Some('’'));
        assert_eq!(standard(0xd0), Some('—'));
        assert_eq!(standard(0xb0), None);
    }

    #[test]
    fn decodes_the_symbol_font() {
        assert_eq!(symbol(b'm'), Some('μ'));
        assert_eq!(symbol(b'W'), Some('Ω'));
        assert_eq!(symbol(0xb1), Some('±'));
        assert_eq!(symbol(0xb0), Some('°'));
        assert_eq!(symbol(0xa3), Some('≤'));
        assert_eq!(symbol(b'~'), Some('∼'));
    }

    #[test]
    fn reads_glyph_names() {
        assert_eq!(glyph_name("Omega").as_deref(), Some("Ω"));
        assert_eq!(glyph_name("uni00B5").as_deref(), Some("µ"));
        assert_eq!(glyph_name("u1F600").as_deref(), Some("😀"));
        assert_eq!(glyph_name("A.sc").as_deref(), Some("A"));
        assert_eq!(glyph_name("f_f_i").as_deref(), Some("ffi"));
        assert_eq!(glyph_name("g123"), None);
    }

    #[test]
    fn reads_text_strings() {
        assert_eq!(text_string(&[0xfe, 0xff, 0, b'P', 0, b'i', 0, b'n']), "Pin");
        assert_eq!(text_string(b"Datasheet \x84 Rev. A"), "Datasheet — Rev. A");
    }
}
