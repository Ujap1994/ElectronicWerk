//! Reading PDF files made by the writer in every variant real datasheets
//! come in.

#![allow(clippy::float_cmp, reason = "rules are rounded to exact values")]

use std::rc::Rc;

use ew_datasheet::pdf::object::{Object, Stream};
use ew_datasheet::pdf::write::{Encryption, WriteOptions, Writer, dict, name, numbers, reference};
use ew_datasheet::{Datasheet, read};

/// A page of US letter size with Helvetica as F1, Helvetica-Bold as F2 and
/// whatever else `extra` adds to the font resources.
struct Builder {
    writer: Writer,
    pages: Vec<u32>,
    pages_root: u32,
    fonts: Vec<(String, Object)>,
}

impl Builder {
    fn new(options: WriteOptions) -> Self {
        let mut writer = Writer::new(options);
        let pages_root = writer.reserve();
        let f1 = writer.add(Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name("Helvetica")),
            ("Encoding", name("WinAnsiEncoding")),
        ])));
        let f2 = writer.add(Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name("Helvetica-Bold")),
            ("Encoding", name("WinAnsiEncoding")),
        ])));
        Self {
            writer,
            pages: Vec::new(),
            pages_root,
            fonts: vec![("F1".into(), reference(f1)), ("F2".into(), reference(f2))],
        }
    }

    fn font(&mut self, key: &str, font: Object) {
        let n = self.writer.add(font);
        self.fonts.push((key.into(), reference(n)));
    }

    fn page(&mut self, content: &str, extra: Vec<(&str, Object)>) -> u32 {
        let fonts = Object::Dict(
            self.fonts
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
                .collect(),
        );
        let contents = self.writer.stream(dict(vec![]), content.as_bytes());
        let mut entries = vec![
            ("Type", name("Page")),
            ("Parent", reference(self.pages_root)),
            ("Contents", reference(contents)),
            ("Resources", Object::Dict(dict(vec![("Font", fonts)]))),
        ];
        entries.extend(extra);
        let page = self.writer.add(Object::Dict(dict(entries)));
        self.pages.push(page);
        page
    }

    fn finish(mut self, catalog_extra: Vec<(&str, Object)>) -> Vec<u8> {
        let kids = Object::Array(self.pages.iter().map(|p| reference(*p)).collect());
        self.writer.set(
            self.pages_root,
            Object::Dict(dict(vec![
                ("Type", name("Pages")),
                ("Kids", kids),
                (
                    "Count",
                    Object::Int(i64::try_from(self.pages.len()).unwrap_or(0)),
                ),
                ("MediaBox", numbers(&[0.0, 0.0, 612.0, 792.0])),
            ])),
        );
        let mut entries = vec![
            ("Type", name("Catalog")),
            ("Pages", reference(self.pages_root)),
        ];
        entries.extend(catalog_extra);
        let catalog = self.writer.add(Object::Dict(dict(entries)));
        let info = self.writer.add(Object::Dict(dict(vec![
            ("Title", Object::String(b"EW321 dual comparator".to_vec())),
            ("Author", Object::String(b"Example Semiconductors".to_vec())),
        ])));
        self.writer.finish(catalog, Some(info))
    }
}

/// A title, a paragraph and a ruled pin table of three rows.
const TABLE_PAGE: &str = "
BT /F2 18 Tf 72 720 Td (EW321) Tj ET
BT /F1 10 Tf 72 700 Td (Dual comparator, 2.7 V to 36 V) Tj ET
0.5 w 72 600 m 372 600 l S
72 580 m 372 580 l S
72 560 m 372 560 l S
72 540 m 372 540 l S
72 540 m 72 600 l S 132 540 m 132 600 l S 232 540 m 232 600 l S 372 540 m 372 600 l S
BT /F2 9 Tf 76 586 Td (PIN) Tj 60 0 Td (NAME) Tj 100 0 Td (FUNCTION) Tj ET
BT /F1 9 Tf 76 566 Td (1) Tj 60 0 Td (OUT1) Tj 100 0 Td [(Output) -250 (of) -250 (comparator) -250 (1)] TJ ET
BT /F1 9 Tf 76 546 Td (8) Tj 60 0 Td (VCC) Tj 100 0 Td (Supply voltage) Tj ET
";

fn line_texts(sheet: &Datasheet, page: usize) -> Vec<String> {
    sheet.pages[page]
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

fn check_table_page(sheet: &Datasheet) {
    assert_eq!(sheet.title, "EW321 dual comparator");
    assert_eq!(sheet.author, "Example Semiconductors");
    let page = &sheet.pages[0];
    assert_eq!((page.width, page.height), (612.0, 792.0));
    assert_eq!(
        line_texts(sheet, 0),
        vec![
            "EW321",
            "Dual comparator, 2.7 V to 36 V",
            "PIN NAME FUNCTION",
            "1 OUT1 Output of comparator 1",
            "8 VCC Supply voltage",
        ]
    );
    // Positions: y down from the top of the page.
    let title = &page.lines[0].words[0];
    assert!((title.bbox[0] - 72.0).abs() < 0.01);
    assert!((title.bbox[3] - (792.0 - 720.0 + 0.2 * 18.0)).abs() < 0.01);
    assert!(title.bold);
    assert!(!page.lines[1].words[0].bold);
    // Helvetica widths: "EW321" = 667 + 944 + 3 * 556 thousandths.
    let width = (667.0 + 944.0 + 3.0 * 556.0) / 1000.0 * 18.0;
    assert!((title.bbox[2] - title.bbox[0] - width).abs() < 0.01);
    // The table: 4 horizontal and 4 vertical rules.
    let horizontal = page.rules.iter().filter(|r| r[1] == r[3]).count();
    let vertical = page.rules.iter().filter(|r| r[0] == r[2]).count();
    assert_eq!((horizontal, vertical), (4, 4));
    assert!(page.rules.contains(&[72.0, 192.0, 372.0, 192.0]));
    let row = &page.lines[3];
    assert_eq!(row.words.len(), 6);
    assert!((row.words[2].bbox[0] - 236.0).abs() < 0.01);
}

#[test]
fn reads_a_plain_file() {
    let mut b = Builder::new(WriteOptions::default());
    b.page(TABLE_PAGE, vec![]);
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    assert!(!sheet.encrypted);
    check_table_page(&sheet);
}

#[test]
fn reads_compressed_files_with_object_streams() {
    let mut b = Builder::new(WriteOptions {
        compress: true,
        object_streams: true,
        encryption: None,
    });
    b.page(TABLE_PAGE, vec![]);
    let data = b.finish(vec![]);
    assert!(data.windows(6).any(|w| w == b"ObjStm"));
    check_table_page(&read(data).unwrap_or_else(|e| panic!("{e}")));
}

#[test]
fn reads_encrypted_files() {
    for encryption in [Encryption::Rc4, Encryption::Aes128] {
        for object_streams in [false, true] {
            let mut b = Builder::new(WriteOptions {
                compress: true,
                object_streams,
                encryption: Some(encryption),
            });
            b.page(TABLE_PAGE, vec![]);
            let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
            assert!(sheet.encrypted);
            check_table_page(&sheet);
        }
    }
}

#[test]
fn recovers_files_with_a_broken_cross_reference() {
    let mut b = Builder::new(WriteOptions::default());
    b.page(TABLE_PAGE, vec![]);
    let data = b.finish(vec![]);
    // Every offset of the table wrong, and the start pointer too.
    let text = String::from_utf8_lossy(&data).into_owned();
    let broken = text.replace("0000000", "0000001");
    let at = broken.rfind("startxref").unwrap_or(0);
    let broken = format!("{}startxref\n99999999\n%%EOF\n", &broken[..at]);
    check_table_page(&read(broken.into_bytes()).unwrap_or_else(|e| panic!("{e}")));
    // No cross-reference at all.
    let at = text.find("xref\n").unwrap_or(text.len());
    check_table_page(&read(text.as_bytes()[..at].to_vec()).unwrap_or_else(|e| panic!("{e}")));
}

#[test]
fn reads_composite_fonts_with_to_unicode() {
    let mut b = Builder::new(WriteOptions {
        compress: true,
        ..WriteOptions::default()
    });
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap
1 begincodespacerange <0000> <FFFF> endcodespacerange
3 beginbfchar <0001> <0056> <0002> <0043> <0003> <00B5> endbfchar
1 beginbfrange <0010> <0019> <0030> endbfrange
endcmap CMapName currentdict /CMap defineresource pop end end";
    let to_unicode = b.writer.stream(dict(vec![]), cmap.as_bytes());
    let descendant = b.writer.add(Object::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("CIDFontType2")),
        ("BaseFont", name("ABCDEF+ArialMT")),
        ("DW", Object::Int(500)),
        (
            "W",
            Object::Array(vec![
                Object::Int(1),
                numbers(&[667.0, 722.0, 576.0]),
                Object::Int(16),
                Object::Int(25),
                Object::Int(556),
            ]),
        ),
    ])));
    b.font(
        "F3",
        Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type0")),
            ("BaseFont", name("ABCDEF+ArialMT")),
            ("Encoding", name("Identity-H")),
            (
                "DescendantFonts",
                Object::Array(vec![reference(descendant)]),
            ),
            ("ToUnicode", reference(to_unicode)),
        ])),
    );
    // "VCC 3.3" would need a dot; "VCC 33 µ".
    b.page(
        "BT /F3 10 Tf 100 700 Td <000100020002> Tj 30 0 Td <00130013> Tj 20 0 Td <0003> Tj ET",
        vec![],
    );
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(line_texts(&sheet, 0), vec!["VCC 33 µ"]);
    let vcc = &sheet.pages[0].lines[0].words[0];
    assert!((vcc.bbox[2] - vcc.bbox[0] - (0.667 + 2.0 * 0.722) * 10.0).abs() < 0.01);
}

#[test]
fn reads_differences_and_the_symbol_font() {
    let mut b = Builder::new(WriteOptions::default());
    b.font(
        "F3",
        Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name("Symbol")),
        ])),
    );
    b.font(
        "F4",
        Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name("Helvetica")),
            (
                "Encoding",
                Object::Dict(dict(vec![
                    ("BaseEncoding", name("WinAnsiEncoding")),
                    (
                        "Differences",
                        Object::Array(vec![Object::Int(1), name("Omega"), name("plusminus")]),
                    ),
                ])),
            ),
        ])),
    );
    b.page(
        "BT /F1 10 Tf 72 700 Td (10 k) Tj /F4 10 Tf (\\001) Tj ET
         BT /F3 10 Tf 72 680 Td (\\261) Tj /F1 10 Tf (5 %) Tj ET
         BT /F3 10 Tf 72 660 Td (m) Tj /F1 10 Tf (A) Tj ET",
        vec![],
    );
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(line_texts(&sheet, 0), vec!["10 kΩ", "±5 %", "μA"]);
}

#[test]
fn places_text_of_rotated_pages_and_rotated_text() {
    let mut b = Builder::new(WriteOptions::default());
    b.page(
        "BT /F1 10 Tf 72 700 Td (Landscape) Tj ET
         BT /F1 8 Tf 0 1 -1 0 300 100 Tm (Up) Tj ET",
        vec![("Rotate", Object::Int(90))],
    );
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    let page = &sheet.pages[0];
    assert_eq!((page.width, page.height), (792.0, 612.0));
    let landscape = page
        .lines
        .iter()
        .find(|l| l.text == "Landscape")
        .unwrap_or_else(|| panic!("no line"));
    // Rotated by a quarter turn: the text runs down the visible page.
    assert_eq!(landscape.angle, 90);
    assert!((landscape.bbox[1] - 72.0).abs() < 0.01);
    // Text turned against the page rotation reads normally.
    let up = page
        .lines
        .iter()
        .find(|l| l.text == "Up")
        .unwrap_or_else(|| panic!("no line"));
    assert_eq!(up.angle, 0);
    assert!((up.bbox[0] - 100.0).abs() < 0.01);
}

#[test]
fn reads_text_in_form_xobjects() {
    let mut b = Builder::new(WriteOptions::default());
    let form = b.writer.add(Object::Stream(Stream {
        dict: dict(vec![
            ("Type", name("XObject")),
            ("Subtype", name("Form")),
            ("BBox", numbers(&[0.0, 0.0, 200.0, 50.0])),
            ("Matrix", numbers(&[1.0, 0.0, 0.0, 1.0, 50.0, 0.0])),
        ]),
        data: Rc::new(b"BT /F1 12 Tf 22 600 Td (Pinout) Tj ET".to_vec()),
    }));
    let fonts = Object::Dict(
        b.fonts
            .iter()
            .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
            .collect(),
    );
    let contents = b.writer.stream(dict(vec![]), b"q /X1 Do Q");
    let page = b.writer.add(Object::Dict(dict(vec![
        ("Type", name("Page")),
        ("Parent", reference(b.pages_root)),
        ("Contents", reference(contents)),
        (
            "Resources",
            Object::Dict(dict(vec![
                ("Font", fonts),
                ("XObject", Object::Dict(dict(vec![("X1", reference(form))]))),
            ])),
        ),
    ])));
    b.pages.push(page);
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    let line = &sheet.pages[0].lines[0];
    assert_eq!(line.text, "Pinout");
    assert!((line.bbox[0] - 72.0).abs() < 0.01);
}

#[test]
fn reads_bookmarks_with_named_destinations() {
    let mut b = Builder::new(WriteOptions::default());
    let first = b.page(TABLE_PAGE, vec![]);
    let second = b.page("BT /F1 10 Tf 72 700 Td (Package outline) Tj ET", vec![]);
    let outlines = b.writer.reserve();
    let item1 = b.writer.reserve();
    let item2 = b.writer.reserve();
    b.writer.set(
        item1,
        Object::Dict(dict(vec![
            ("Title", Object::String(b"Pin Configuration".to_vec())),
            ("Parent", reference(outlines)),
            ("Next", reference(item2)),
            (
                "Dest",
                Object::Array(vec![
                    reference(first),
                    name("XYZ"),
                    Object::Null,
                    Object::Null,
                    Object::Null,
                ]),
            ),
        ])),
    );
    b.writer.set(
        item2,
        Object::Dict(dict(vec![
            ("Title", Object::String(b"Mechanical Data".to_vec())),
            ("Parent", reference(outlines)),
            ("Dest", Object::String(b"mech".to_vec())),
        ])),
    );
    b.writer.set(
        outlines,
        Object::Dict(dict(vec![
            ("Type", name("Outlines")),
            ("First", reference(item1)),
            ("Last", reference(item2)),
        ])),
    );
    let dests = b.writer.add(Object::Dict(dict(vec![(
        "Names",
        Object::Array(vec![
            Object::String(b"mech".to_vec()),
            Object::Array(vec![reference(second), name("Fit")]),
        ]),
    )])));
    let names = Object::Dict(dict(vec![("Dests", reference(dests))]));
    let sheet = read(b.finish(vec![("Outlines", reference(outlines)), ("Names", names)]))
        .unwrap_or_else(|e| panic!("{e}"));
    let marks: Vec<(String, Option<usize>)> = sheet
        .bookmarks
        .iter()
        .map(|m| (m.title.clone(), m.page))
        .collect();
    assert_eq!(
        marks,
        vec![
            ("Pin Configuration".to_owned(), Some(1)),
            ("Mechanical Data".to_owned(), Some(2)),
        ]
    );
}

#[test]
fn splits_words_at_gaps_and_joins_subscripts() {
    let mut b = Builder::new(WriteOptions::default());
    b.page(
        // Kerned text with word gaps, and V with a subscript CC.
        "BT /F1 10 Tf 72 700 Td [(Sup) 30 (ply) -280 (voltage)] TJ ET
         BT /F1 10 Tf 72 680 Td (V) Tj 6.67 -2 Td /F1 7 Tf (CC) Tj ET",
        vec![],
    );
    let sheet = read(b.finish(vec![])).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(line_texts(&sheet, 0), vec!["Supply voltage", "VCC"]);
}

#[test]
fn refuses_files_that_are_not_pdf() {
    assert!(read(b"hello".to_vec()).is_err());
}
