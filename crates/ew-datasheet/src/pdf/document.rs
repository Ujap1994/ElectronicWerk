//! The file structure: cross-reference tables and streams, object streams,
//! encryption, the page tree, document information and bookmarks. Broken
//! files are read by scanning for objects, as viewers do.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::crypt::{CryptError, Decryptor};
use super::encoding::text_string;
use super::filter;
use super::object::{Dict, Lexer, Object, Stream, Token, is_white};

/// Why a file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PdfError {
    /// Not a PDF file.
    #[error("not a PDF file")]
    NotPdf,
    /// No page tree could be found.
    #[error("the PDF file has no pages")]
    NoPages,
    /// Encrypted in a way that cannot be opened.
    #[error("{0}")]
    Crypt(#[from] CryptError),
}

/// Objects of an object stream: number and offset in the decoded data.
type ObjectList = Vec<(u32, usize)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Offset(usize),
    Compressed(u32, u32),
}

/// A page of the page tree with its inherited attributes.
#[derive(Debug, Clone)]
pub struct PageRef {
    /// Object number of the page (to resolve bookmarks).
    pub number: Option<u32>,
    /// The page dictionary with `Resources`, `MediaBox`, `CropBox` and
    /// `Rotate` inherited from its parents.
    pub dict: Dict,
}

/// A bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    /// Title.
    pub title: String,
    /// Nesting level (0 at the top).
    pub level: usize,
    /// Page index (0-based), when known.
    pub page: Option<usize>,
}

/// An open PDF file.
#[derive(Debug)]
pub struct Document {
    data: Vec<u8>,
    xref: HashMap<u32, Entry>,
    trailer: Dict,
    crypt: Option<Decryptor>,
    encrypt_ref: Option<u32>,
    cache: RefCell<HashMap<u32, Object>>,
    streams: RefCell<HashMap<u32, Rc<ObjectList>>>,
    decoded: RefCell<HashMap<u32, Rc<Vec<u8>>>>,
    scanned: RefCell<Option<HashMap<u32, usize>>>,
    depth: Cell<u32>,
}

fn find_last(data: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if data.len() < needle.len() {
        return None;
    }
    (from.min(data.len() - needle.len())..=data.len() - needle.len())
        .rev()
        .find(|&i| &data[i..i + needle.len()] == needle)
}

fn find(data: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= data.len() {
        return None;
    }
    data[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

impl Document {
    /// Opens a PDF file from its bytes.
    ///
    /// # Errors
    ///
    /// Not a PDF file, no pages, or encrypted with a password.
    pub fn open(data: Vec<u8>) -> Result<Self, PdfError> {
        let header = find(&data[..data.len().min(1024)], b"%PDF-", 0).ok_or(PdfError::NotPdf)?;
        let mut doc = Self {
            data,
            xref: HashMap::new(),
            trailer: Dict::new(),
            crypt: None,
            encrypt_ref: None,
            cache: RefCell::new(HashMap::new()),
            streams: RefCell::new(HashMap::new()),
            decoded: RefCell::new(HashMap::new()),
            scanned: RefCell::new(None),
            depth: Cell::new(0),
        };
        if !doc.read_xref(header) || !doc.trailer.contains_key(b"Root".as_slice()) {
            doc.rebuild();
        }
        if let Some(encrypt) = doc.trailer.get(b"Encrypt".as_slice()).cloned() {
            if let Object::Ref(n, _) = encrypt {
                doc.encrypt_ref = Some(n);
            }
            let dict = doc.resolve(&encrypt);
            if let Some(dict) = dict.dict() {
                let id0 = doc
                    .trailer
                    .get(b"ID".as_slice())
                    .and_then(Object::array)
                    .and_then(|a| a.first())
                    .and_then(Object::string)
                    .map(<[u8]>::to_vec)
                    .unwrap_or_default();
                doc.crypt = Some(Decryptor::new(dict, &id0)?);
                doc.cache.borrow_mut().clear();
                doc.streams.borrow_mut().clear();
                doc.decoded.borrow_mut().clear();
            }
        }
        if doc.pages().is_empty() {
            // A broken page tree: try the scanned objects.
            if doc.scanned.borrow().is_none() {
                doc.rebuild();
            }
            if doc.pages().is_empty() {
                return Err(PdfError::NoPages);
            }
        }
        Ok(doc)
    }

    /// The trailer dictionary.
    #[must_use]
    pub const fn trailer(&self) -> &Dict {
        &self.trailer
    }

    /// Whether the file was encrypted.
    #[must_use]
    pub const fn encrypted(&self) -> bool {
        self.crypt.is_some()
    }

    // ----- cross-reference ------------------------------------------------

    fn read_xref(&mut self, header: usize) -> bool {
        let tail = self.data.len().saturating_sub(4096);
        let Some(at) = find_last(&self.data, b"startxref", tail.max(1).saturating_sub(1)) else {
            return false;
        };
        let mut lexer = Lexer::new(&self.data, at + 9);
        let Some(Token::Int(mut offset)) = lexer.token() else {
            return false;
        };
        let mut seen = HashSet::new();
        let mut first = true;
        while offset >= 0 && seen.insert(offset) {
            let mut pos = offset as usize;
            if !self.xref_at(pos) && header > 0 {
                pos += header;
            }
            let Some((trailer, entries)) = self.read_section(pos) else {
                return !first && !self.xref.is_empty();
            };
            self.add_entries(entries);
            if let Some(stm) = trailer.get(b"XRefStm".as_slice()).and_then(Object::int)
                && let Some((_, entries)) = self.read_section(stm.max(0) as usize)
            {
                self.add_entries(entries);
            }
            if first {
                self.trailer = trailer.clone();
                first = false;
            } else {
                for (k, v) in &trailer {
                    self.trailer.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            match trailer.get(b"Prev".as_slice()).and_then(Object::int) {
                Some(prev) => offset = prev,
                None => break,
            }
        }
        !self.xref.is_empty()
    }

    fn xref_at(&self, pos: usize) -> bool {
        let mut lexer = Lexer::new(&self.data, pos);
        match lexer.token() {
            Some(Token::Keyword(b"xref")) => true,
            Some(Token::Int(_)) => {
                matches!(lexer.token(), Some(Token::Int(_)))
                    && matches!(lexer.token(), Some(Token::Keyword(b"obj")))
            }
            _ => false,
        }
    }

    fn add_entries(&mut self, entries: Vec<(u32, Entry)>) {
        for (n, e) in entries {
            self.xref.entry(n).or_insert(e);
        }
    }

    /// Reads one cross-reference section (table or stream): its trailer
    /// and entries.
    fn read_section(&self, pos: usize) -> Option<(Dict, Vec<(u32, Entry)>)> {
        let mut lexer = Lexer::new(&self.data, pos);
        match lexer.token()? {
            Token::Keyword(b"xref") => Self::read_table(&mut lexer),
            Token::Int(_) => {
                lexer.token()?;
                if lexer.token()? != Token::Keyword(b"obj") {
                    return None;
                }
                let object = self.parse_after_obj(&mut lexer, None);
                let stream = object.stream()?;
                Some((stream.dict.clone(), Self::read_xref_stream(stream)))
            }
            _ => None,
        }
    }

    fn read_table(lexer: &mut Lexer<'_>) -> Option<(Dict, Vec<(u32, Entry)>)> {
        let mut entries = Vec::new();
        loop {
            let save = lexer.pos;
            match lexer.token()? {
                Token::Int(start) => {
                    let Token::Int(count) = lexer.token()? else {
                        return None;
                    };
                    for i in 0..count.max(0) {
                        let Some(Token::Int(offset)) = lexer.token() else {
                            return None;
                        };
                        let _generation = lexer.token();
                        let kind = lexer.token();
                        let number = (start + i) as u32;
                        if kind == Some(Token::Keyword(b"n")) && offset > 0 {
                            entries.push((number, Entry::Offset(offset as usize)));
                        }
                    }
                }
                Token::Keyword(b"trailer") => {
                    return lexer
                        .object(true)
                        .and_then(|o| o.dict().cloned())
                        .map(|d| (d, entries));
                }
                _ => {
                    lexer.pos = save;
                    return None;
                }
            }
        }
    }

    fn read_xref_stream(stream: &Stream) -> Vec<(u32, Entry)> {
        let mut out = Vec::new();
        let widths: Vec<usize> = stream
            .dict
            .get(b"W".as_slice())
            .map(Object::numbers)
            .unwrap_or_default()
            .iter()
            .map(|w| (*w as usize).min(8))
            .collect();
        if widths.len() < 3 {
            return out;
        }
        let size = stream
            .dict
            .get(b"Size".as_slice())
            .and_then(Object::int)
            .unwrap_or(0);
        let index = stream
            .dict
            .get(b"Index".as_slice())
            .map_or_else(|| vec![0.0, size as f64], Object::numbers);
        let data = filter::decode(&stream.dict, &stream.data);
        let row: usize = widths.iter().sum();
        if row == 0 {
            return out;
        }
        let read = |bytes: &[u8]| bytes.iter().fold(0u64, |v, &b| v << 8 | u64::from(b));
        let mut rows = data.chunks_exact(row);
        for pair in index.chunks_exact(2) {
            let (start, count) = (pair[0] as u32, pair[1] as u32);
            for i in 0..count {
                let Some(r) = rows.next() else {
                    return out;
                };
                let (a, rest) = r.split_at(widths[0]);
                let (b, c) = rest.split_at(widths[1]);
                let kind = if widths[0] == 0 { 1 } else { read(a) };
                let entry = match kind {
                    1 => Entry::Offset(read(b) as usize),
                    2 => Entry::Compressed(read(b) as u32, read(c) as u32),
                    _ => continue,
                };
                out.push((start + i, entry));
            }
        }
        out
    }

    /// Rebuilds the cross-reference by scanning the file for objects.
    fn rebuild(&mut self) {
        let offsets = self.scan();
        for (&n, &offset) in &offsets {
            self.xref.insert(n, Entry::Offset(offset));
        }
        // Objects inside object streams.
        let mut compressed = Vec::new();
        for &n in offsets.keys() {
            if let Some(Object::Stream(s)) = self.get(n)
                && s.dict.get(b"Type".as_slice()).and_then(Object::name) == Some(b"ObjStm")
                && let Some(list) = self.object_stream(n)
            {
                for (index, (number, _)) in list.iter().enumerate() {
                    if !offsets.contains_key(number) {
                        compressed.push((*number, n, index as u32));
                    }
                }
            }
        }
        for (number, stream, index) in compressed {
            self.xref
                .entry(number)
                .or_insert(Entry::Compressed(stream, index));
        }
        self.cache.borrow_mut().clear();
        // The trailer: the last `trailer` dictionary, else a catalog.
        let mut at = 0;
        while let Some(found) = find(&self.data, b"trailer", at) {
            at = found + 7;
            if let Some(Object::Dict(d)) = Lexer::new(&self.data, at).object(true)
                && d.contains_key(b"Root".as_slice())
            {
                for (k, v) in d {
                    self.trailer.insert(k, v);
                }
            }
        }
        if !self.trailer.contains_key(b"Root".as_slice()) {
            let mut numbers: Vec<u32> = self.xref.keys().copied().collect();
            numbers.sort_unstable();
            for n in numbers {
                let Some(object) = self.get(n) else {
                    continue;
                };
                let Some(dict) = object.dict() else {
                    continue;
                };
                match dict.get(b"Type".as_slice()).and_then(Object::name) {
                    Some(b"XRef") => {
                        for key in ["Root", "Info", "ID", "Encrypt"] {
                            if let Some(v) = dict.get(key.as_bytes()) {
                                self.trailer.insert(key.as_bytes().to_vec(), v.clone());
                            }
                        }
                    }
                    Some(b"Catalog") => {
                        self.trailer
                            .entry(b"Root".to_vec())
                            .or_insert(Object::Ref(n, 0));
                    }
                    None if ["Title", "Author", "Producer", "Creator", "CreationDate"]
                        .iter()
                        .any(|k| dict.contains_key(k.as_bytes())) =>
                    {
                        // Looks like the document information.
                        self.trailer
                            .entry(b"Info".to_vec())
                            .or_insert(Object::Ref(n, 0));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Offsets of `n g obj` in the whole file (the last one of a number
    /// wins, as in incremental updates).
    fn scan(&self) -> HashMap<u32, usize> {
        if let Some(found) = self.scanned.borrow().as_ref() {
            return found.clone();
        }
        let data = &self.data;
        let mut out = HashMap::new();
        let mut at = 0;
        while let Some(found) = find(data, b"obj", at) {
            at = found + 3;
            if data
                .get(found + 3)
                .is_some_and(|&c| !is_white(c) && c != b'<' && c != b'[' && c != b'/' && c != b'%')
            {
                continue;
            }
            // Walk back over `gen` and `num`.
            let mut i = found;
            let skip_white = |i: &mut usize| {
                while *i > 0 && is_white(data[*i - 1]) {
                    *i -= 1;
                }
            };
            let digits = |i: &mut usize| {
                let end = *i;
                while *i > 0 && data[*i - 1].is_ascii_digit() {
                    *i -= 1;
                }
                (*i < end)
                    .then(|| {
                        std::str::from_utf8(&data[*i..end])
                            .ok()
                            .and_then(|s| s.parse::<u64>().ok())
                    })
                    .flatten()
            };
            skip_white(&mut i);
            if digits(&mut i).is_none() {
                continue;
            }
            let before_gen = i;
            skip_white(&mut i);
            if i == before_gen {
                continue;
            }
            let Some(number) = digits(&mut i) else {
                continue;
            };
            if i > 0 && !is_white(data[i - 1]) && !matches!(data[i - 1], b'>' | b')' | b']') {
                continue;
            }
            if let Ok(number) = u32::try_from(number) {
                out.insert(number, i);
            }
        }
        *self.scanned.borrow_mut() = Some(out.clone());
        out
    }

    // ----- objects ----------------------------------------------------------

    /// Reads the object after `n g obj`; decrypts it when `id` is given.
    fn parse_after_obj(&self, lexer: &mut Lexer<'_>, id: Option<(u32, u16)>) -> Object {
        let object = lexer.object(true).unwrap_or(Object::Null);
        let save = lexer.pos;
        if let (Object::Dict(dict), Some(Token::Keyword(b"stream"))) = (&object, lexer.token()) {
            let mut start = lexer.pos;
            if self.data.get(start) == Some(&b'\r') {
                start += 1;
            }
            if self.data.get(start) == Some(&b'\n') {
                start += 1;
            }
            let length = dict
                .get(b"Length".as_slice())
                .map(|l| match l {
                    Object::Ref(..) => self.resolve(l),
                    other => other.clone(),
                })
                .and_then(|l| l.int())
                .unwrap_or(-1);
            let fits = length >= 0 && {
                let end = start + length as usize;
                let mut check = Lexer::new(&self.data, end.min(self.data.len()));
                check.skip_white();
                self.data[check.pos..].starts_with(b"endstream")
            };
            let end = if fits {
                start + length as usize
            } else {
                let found = find(&self.data, b"endstream", start).unwrap_or(self.data.len());
                let mut end = found;
                if end > start && self.data[end - 1] == b'\n' {
                    end -= 1;
                }
                if end > start && self.data[end - 1] == b'\r' {
                    end -= 1;
                }
                end
            };
            let raw = &self.data[start.min(end)..end];
            let data = match (id, &self.crypt) {
                (Some((n, g)), Some(crypt)) if !is_xref(dict) => crypt.decrypt(n, g, raw, true),
                _ => raw.to_vec(),
            };
            let mut dict = dict.clone();
            if let (Some((n, g)), Some(crypt)) = (id, &self.crypt) {
                decrypt_strings_in_dict(&mut dict, crypt, n, g);
            }
            Object::Stream(Stream {
                dict,
                data: Rc::new(data),
            })
        } else {
            lexer.pos = save;
            match (id, &self.crypt) {
                (Some((n, g)), Some(crypt)) => {
                    let mut o = object;
                    decrypt_strings(&mut o, crypt, n, g);
                    o
                }
                _ => object,
            }
        }
    }

    fn object_at(&self, number: u32, offset: usize) -> Option<Object> {
        let mut lexer = Lexer::new(&self.data, offset);
        let Some(Token::Int(n)) = lexer.token() else {
            return None;
        };
        let Some(Token::Int(generation)) = lexer.token() else {
            return None;
        };
        if n != i64::from(number) || lexer.token() != Some(Token::Keyword(b"obj")) {
            return None;
        }
        let id = (self.encrypt_ref != Some(number)).then_some((number, generation as u16));
        Some(self.parse_after_obj(&mut lexer, id))
    }

    /// Objects of an object stream: (number, offset in the decoded data).
    fn object_stream(&self, number: u32) -> Option<Rc<ObjectList>> {
        if let Some(list) = self.streams.borrow().get(&number) {
            return Some(Rc::clone(list));
        }
        let Some(Object::Stream(stream)) = self.get(number) else {
            return None;
        };
        let data = Rc::new(filter::decode(&stream.dict, &stream.data));
        let first = stream
            .dict
            .get(b"First".as_slice())
            .and_then(Object::int)
            .unwrap_or(0)
            .max(0) as usize;
        let count = stream
            .dict
            .get(b"N".as_slice())
            .and_then(Object::int)
            .unwrap_or(0)
            .max(0);
        let mut lexer = Lexer::new(&data, 0);
        let mut list = Vec::new();
        for _ in 0..count {
            let (Some(Token::Int(n)), Some(Token::Int(offset))) = (lexer.token(), lexer.token())
            else {
                break;
            };
            list.push((n as u32, first + offset.max(0) as usize));
        }
        let list = Rc::new(list);
        self.streams.borrow_mut().insert(number, Rc::clone(&list));
        self.decoded.borrow_mut().insert(number, data);
        Some(list)
    }

    fn compressed(&self, number: u32, stream: u32, index: u32) -> Option<Object> {
        let list = self.object_stream(stream)?;
        let data = Rc::clone(self.decoded.borrow().get(&stream)?);
        let offset = list
            .get(index as usize)
            .filter(|(n, _)| *n == number)
            .or_else(|| list.iter().find(|(n, _)| *n == number))
            .map(|(_, o)| *o)?;
        Lexer::new(&data, offset).object(true)
    }

    /// An indirect object by its number.
    #[must_use]
    pub fn get(&self, number: u32) -> Option<Object> {
        if let Some(o) = self.cache.borrow().get(&number) {
            return Some(o.clone());
        }
        if self.depth.get() > 32 {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let object = match self.xref.get(&number).copied() {
            Some(Entry::Offset(offset)) => self.object_at(number, offset).or_else(|| {
                // A wrong offset: use the scanned position.
                let scanned = self.scan();
                scanned
                    .get(&number)
                    .and_then(|&o| self.object_at(number, o))
            }),
            Some(Entry::Compressed(stream, index)) => self.compressed(number, stream, index),
            None => {
                let scanned = self.scan();
                scanned
                    .get(&number)
                    .and_then(|&o| self.object_at(number, o))
            }
        };
        self.depth.set(self.depth.get() - 1);
        if let Some(o) = &object {
            self.cache.borrow_mut().insert(number, o.clone());
        }
        object
    }

    /// The object itself, following references.
    #[must_use]
    pub fn resolve(&self, object: &Object) -> Object {
        let mut current = object.clone();
        for _ in 0..8 {
            match current {
                Object::Ref(n, _) => current = self.get(n).unwrap_or(Object::Null),
                other => return other,
            }
        }
        Object::Null
    }

    /// An entry of a dictionary, following references.
    #[must_use]
    pub fn entry(&self, dict: &Dict, key: &str) -> Object {
        dict.get(key.as_bytes())
            .map_or(Object::Null, |o| self.resolve(o))
    }

    /// The decoded data of a stream.
    #[must_use]
    pub fn decode(&self, stream: &Stream) -> Vec<u8> {
        filter::decode(&stream.dict, &stream.data)
    }

    // ----- pages ------------------------------------------------------------

    /// The pages in order.
    #[must_use]
    pub fn pages(&self) -> Vec<PageRef> {
        let root = self.entry(&self.trailer, "Root");
        let mut out = Vec::new();
        if let Some(root) = root.dict() {
            let pages = root
                .get(b"Pages".as_slice())
                .cloned()
                .unwrap_or(Object::Null);
            let mut seen = HashSet::new();
            self.walk(&pages, &Dict::new(), &mut out, &mut seen, 0);
        }
        if out.is_empty() {
            // No usable tree: every page object in file order.
            let scanned = self.scan();
            let mut found: Vec<(usize, u32)> = scanned.iter().map(|(n, o)| (*o, *n)).collect();
            found.sort_unstable();
            for (_, n) in found {
                if let Some(Object::Dict(d)) = self.get(n)
                    && d.get(b"Type".as_slice()).and_then(Object::name) == Some(b"Page")
                {
                    out.push(PageRef {
                        number: Some(n),
                        dict: d,
                    });
                }
            }
        }
        out
    }

    fn walk(
        &self,
        node: &Object,
        inherited: &Dict,
        out: &mut Vec<PageRef>,
        seen: &mut HashSet<u32>,
        depth: usize,
    ) {
        if depth > 64 || out.len() > 10_000 {
            return;
        }
        let number = match node {
            Object::Ref(n, _) => {
                if !seen.insert(*n) {
                    return;
                }
                Some(*n)
            }
            _ => None,
        };
        let resolved = self.resolve(node);
        let Some(dict) = resolved.dict() else {
            return;
        };
        let mut attributes = inherited.clone();
        for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
            if let Some(v) = dict.get(key.as_bytes()) {
                attributes.insert(key.as_bytes().to_vec(), v.clone());
            }
        }
        let kids = self.entry(dict, "Kids");
        let is_tree = dict.get(b"Type".as_slice()).and_then(Object::name) == Some(b"Pages")
            || (kids.array().is_some()
                && dict.get(b"Type".as_slice()).and_then(Object::name) != Some(b"Page"));
        if is_tree {
            for kid in kids.array().unwrap_or_default() {
                self.walk(kid, &attributes, out, seen, depth + 1);
            }
        } else {
            let mut page = dict.clone();
            for (k, v) in attributes {
                page.entry(k).or_insert(v);
            }
            out.push(PageRef { number, dict: page });
        }
    }

    /// The decoded content of a page (all content streams joined).
    #[must_use]
    pub fn contents(&self, page: &Dict) -> Vec<u8> {
        let mut out = Vec::new();
        let contents = self.entry(page, "Contents");
        let parts: Vec<Object> = match &contents {
            Object::Array(items) => items.iter().map(|o| self.resolve(o)).collect(),
            other => vec![other.clone()],
        };
        for part in parts {
            if let Object::Stream(s) = part {
                out.extend(self.decode(&s));
                out.push(b'\n');
            }
        }
        out
    }

    // ----- information ------------------------------------------------------

    /// A text entry of the document information (`Title`, `Author` …).
    #[must_use]
    pub fn info(&self, key: &str) -> String {
        let info = self.entry(&self.trailer, "Info");
        info.dict()
            .map(|d| self.entry(d, key))
            .and_then(|o| o.string().map(text_string))
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    }

    /// The bookmarks with their pages.
    #[must_use]
    pub fn bookmarks(&self, pages: &[PageRef]) -> Vec<Bookmark> {
        let index: HashMap<u32, usize> = pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.number.map(|n| (n, i)))
            .collect();
        let root = self.entry(&self.trailer, "Root");
        let Some(root) = root.dict() else {
            return Vec::new();
        };
        let outlines = self.entry(root, "Outlines");
        let Some(outlines) = outlines.dict() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.outline_level(
            outlines.get(b"First".as_slice()),
            0,
            root,
            &index,
            &mut out,
            &mut seen,
        );
        out
    }

    fn outline_level(
        &self,
        first: Option<&Object>,
        level: usize,
        root: &Dict,
        index: &HashMap<u32, usize>,
        out: &mut Vec<Bookmark>,
        seen: &mut HashSet<u32>,
    ) {
        let mut current = first.cloned();
        while let Some(Object::Ref(n, _)) = current {
            if !seen.insert(n) || out.len() > 5_000 || level > 16 {
                return;
            }
            let item = self.resolve(&Object::Ref(n, 0));
            let Some(dict) = item.dict() else {
                return;
            };
            let title = self
                .entry(dict, "Title")
                .string()
                .map(text_string)
                .unwrap_or_default();
            let mut destination = self.entry(dict, "Dest");
            if destination == Object::Null
                && let Some(action) = self.entry(dict, "A").dict()
            {
                destination = self.entry(action, "D");
            }
            let page = self.destination_page(&destination, root, index);
            out.push(Bookmark {
                title: title.trim().to_owned(),
                level,
                page,
            });
            self.outline_level(
                dict.get(b"First".as_slice()),
                level + 1,
                root,
                index,
                out,
                seen,
            );
            current = dict.get(b"Next".as_slice()).cloned();
        }
    }

    fn destination_page(
        &self,
        destination: &Object,
        root: &Dict,
        index: &HashMap<u32, usize>,
    ) -> Option<usize> {
        let resolved = match destination {
            Object::Name(name) => {
                let dests = self.entry(root, "Dests");
                dests
                    .dict()
                    .map(|d| self.entry(d, &String::from_utf8_lossy(name)))?
            }
            Object::String(name) => {
                let names = self.entry(root, "Names");
                let tree = names.dict().map(|d| self.entry(d, "Dests"))?;
                self.name_tree(&tree, name, 0)?
            }
            other => other.clone(),
        };
        let resolved = match resolved {
            Object::Dict(d) => self.entry(&d, "D"),
            other => other,
        };
        match resolved.array()?.first()? {
            Object::Ref(n, _) => index.get(n).copied(),
            Object::Int(i) => usize::try_from(*i).ok(),
            _ => None,
        }
    }

    fn name_tree(&self, node: &Object, key: &[u8], depth: usize) -> Option<Object> {
        if depth > 32 {
            return None;
        }
        let dict = node.dict()?;
        if let Some(names) = self.entry(dict, "Names").array() {
            for pair in names.chunks_exact(2) {
                if pair[0].string() == Some(key) {
                    return Some(self.resolve(&pair[1]));
                }
            }
        }
        for kid in self.entry(dict, "Kids").array().unwrap_or_default() {
            let kid = self.resolve(kid);
            let in_range = kid
                .dict()
                .and_then(|d| d.get(b"Limits".as_slice()))
                .and_then(Object::array)
                .is_none_or(|limits| {
                    let low = limits.first().and_then(Object::string).unwrap_or_default();
                    let high = limits.get(1).and_then(Object::string).unwrap_or_default();
                    low <= key && key <= high
                });
            if in_range && let Some(found) = self.name_tree(&kid, key, depth + 1) {
                return Some(found);
            }
        }
        None
    }
}

fn is_xref(dict: &Dict) -> bool {
    dict.get(b"Type".as_slice()).and_then(Object::name) == Some(b"XRef")
}

fn decrypt_strings(object: &mut Object, crypt: &Decryptor, number: u32, generation: u16) {
    match object {
        Object::String(s) => *s = crypt.decrypt(number, generation, s, false),
        Object::Array(items) => {
            for item in items {
                decrypt_strings(item, crypt, number, generation);
            }
        }
        Object::Dict(d) => decrypt_strings_in_dict(d, crypt, number, generation),
        _ => {}
    }
}

fn decrypt_strings_in_dict(dict: &mut Dict, crypt: &Decryptor, number: u32, generation: u16) {
    for value in dict.values_mut() {
        decrypt_strings(value, crypt, number, generation);
    }
}
