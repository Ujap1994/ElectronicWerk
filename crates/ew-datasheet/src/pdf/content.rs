//! The content stream interpreter: places every character and every
//! straight line of a page in display space (points, origin at the top
//! left of the visible page, y down, page rotation applied).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::document::{Document, PageRef};
use super::font::Font;
use super::object::{Dict, Lexer, Object, Token, is_white};

/// An affine matrix `[a b c d e f]` as in PDF.
pub type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m1` then `m2`.
#[must_use]
pub fn multiply(m1: &Matrix, m2: &Matrix) -> Matrix {
    [
        m1[0] * m2[0] + m1[1] * m2[2],
        m1[0] * m2[1] + m1[1] * m2[3],
        m1[2] * m2[0] + m1[3] * m2[2],
        m1[2] * m2[1] + m1[3] * m2[3],
        m1[4] * m2[0] + m1[5] * m2[2] + m2[4],
        m1[4] * m2[1] + m1[5] * m2[3] + m2[5],
    ]
}

fn apply(m: &Matrix, x: f64, y: f64) -> (f64, f64) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

/// One placed character.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    /// Its text.
    pub text: String,
    /// Baseline start.
    pub x: f64,
    /// Baseline start.
    pub y: f64,
    /// Advance of the visible glyph (baseline direction and length).
    pub dx: f64,
    /// Advance of the visible glyph.
    pub dy: f64,
    /// Font size on the page.
    pub size: f64,
    /// Direction of the baseline (radians, display space, y down).
    pub angle: f64,
    /// Index into [`PageContent::fonts`].
    pub font: usize,
    /// Drawn invisibly (a text layer over a scanned image).
    pub invisible: bool,
}

/// A straight horizontal or vertical line (`x0 <= x1`, `y0 <= y1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Start.
    pub x0: f64,
    /// Start.
    pub y0: f64,
    /// End.
    pub x1: f64,
    /// End.
    pub y1: f64,
}

/// What a page shows.
#[derive(Debug, Clone, Default)]
pub struct PageContent {
    /// Visible page width (points).
    pub width: f64,
    /// Visible page height (points).
    pub height: f64,
    /// Characters in drawing order.
    pub glyphs: Vec<PlacedGlyph>,
    /// Horizontal and vertical lines (table rules, cell borders).
    pub segments: Vec<Segment>,
    /// Bounds of images `[x0, y0, x1, y1]`.
    pub images: Vec<[f64; 4]>,
    /// Fonts used: name and bold.
    pub fonts: Vec<(String, bool)>,
}

#[derive(Debug, Clone)]
struct State {
    ctm: Matrix,
    font: Option<Rc<Font>>,
    font_index: usize,
    size: f64,
    char_space: f64,
    word_space: f64,
    scale: f64,
    leading: f64,
    rise: f64,
    render: i64,
    fill_white: bool,
    stroke_white: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ctm: IDENTITY,
            font: None,
            font_index: 0,
            size: 0.0,
            char_space: 0.0,
            word_space: 0.0,
            scale: 1.0,
            leading: 0.0,
            rise: 0.0,
            render: 0,
            fill_white: false,
            stroke_white: false,
        }
    }
}

/// Reads fonts once per document.
#[derive(Debug, Default)]
pub struct FontCache {
    by_object: HashMap<u32, (Rc<Font>, usize)>,
    fonts: Vec<(String, bool)>,
}

/// The display transform of a page and its visible size.
#[must_use]
pub fn page_transform(doc: &Document, page: &PageRef) -> (Matrix, f64, f64) {
    let media = doc.entry(&page.dict, "MediaBox").numbers();
    let media = if media.len() == 4 {
        media
    } else {
        vec![0.0, 0.0, 612.0, 792.0]
    };
    let crop = doc.entry(&page.dict, "CropBox").numbers();
    let normalize = |b: &[f64]| {
        [
            b[0].min(b[2]),
            b[1].min(b[3]),
            b[0].max(b[2]),
            b[1].max(b[3]),
        ]
    };
    let mb = normalize(&media);
    let mut bx = if crop.len() == 4 {
        let cb = normalize(&crop);
        [
            cb[0].max(mb[0]),
            cb[1].max(mb[1]),
            cb[2].min(mb[2]),
            cb[3].min(mb[3]),
        ]
    } else {
        mb
    };
    if bx[2] - bx[0] < 1.0 || bx[3] - bx[1] < 1.0 {
        bx = mb;
    }
    let [llx, lly, urx, ury] = bx;
    let (w, h) = (urx - llx, ury - lly);
    let rotate = doc
        .entry(&page.dict, "Rotate")
        .int()
        .unwrap_or(0)
        .rem_euclid(360)
        / 90
        * 90;
    match rotate {
        90 => ([0.0, 1.0, 1.0, 0.0, -lly, -llx], h, w),
        180 => ([-1.0, 0.0, 0.0, 1.0, urx, -lly], w, h),
        270 => ([0.0, -1.0, -1.0, 0.0, ury, urx], h, w),
        _ => ([1.0, 0.0, 0.0, -1.0, -llx, ury], w, h),
    }
}

/// Interprets a page.
#[must_use]
pub fn interpret(doc: &Document, page: &PageRef, fonts: &mut FontCache) -> PageContent {
    let (transform, width, height) = page_transform(doc, page);
    let mut run = Run {
        doc,
        fonts,
        out: PageContent {
            width,
            height,
            ..PageContent::default()
        },
        forms: HashSet::new(),
        operations: 0,
    };
    let resources = doc
        .entry(&page.dict, "Resources")
        .dict()
        .cloned()
        .unwrap_or_default();
    let content = doc.contents(&page.dict);
    let state = State {
        ctm: transform,
        ..State::default()
    };
    run.content(&content, &resources, state, 0);
    run.out.fonts.clone_from(&run.fonts.fonts);
    run.out
}

struct Run<'a> {
    doc: &'a Document,
    fonts: &'a mut FontCache,
    out: PageContent,
    forms: HashSet<u32>,
    operations: usize,
}

/// Upper limit of operators per page (broken or hostile files).
const MAX_OPERATIONS: usize = 5_000_000;

fn is_white_color(values: &[f64], cmyk: bool) -> bool {
    if values.is_empty() {
        return false;
    }
    if cmyk {
        values.iter().all(|v| *v <= 0.02)
    } else {
        values.iter().all(|v| *v >= 0.98)
    }
}

impl Run<'_> {
    fn font(&mut self, resources: &Dict, name: &[u8]) -> (Rc<Font>, usize) {
        let fonts = self.doc.entry(resources, "Font");
        let reference = fonts.dict().and_then(|d| d.get(name)).cloned();
        if let Some(Object::Ref(n, _)) = reference
            && let Some((font, index)) = self.fonts.by_object.get(&n)
        {
            return (Rc::clone(font), *index);
        }
        let font = match reference.as_ref().map(|r| self.doc.resolve(r)) {
            Some(Object::Dict(d)) => Font::load(self.doc, &d),
            _ => Font::fallback(),
        };
        let index = self.fonts.fonts.len();
        self.fonts.fonts.push((font.name.clone(), font.bold));
        let font = Rc::new(font);
        if let Some(Object::Ref(n, _)) = reference {
            self.fonts.by_object.insert(n, (Rc::clone(&font), index));
        }
        (font, index)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one match over the PDF operators, named as in the PDF reference"
    )]
    fn content(&mut self, data: &[u8], resources: &Dict, initial: State, depth: usize) {
        let mut lexer = Lexer::new(data, 0);
        let mut operands: Vec<Object> = Vec::new();
        let mut state = initial;
        let mut stack: Vec<State> = Vec::new();
        let mut tm = IDENTITY;
        let mut tlm = IDENTITY;
        let mut path: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut current = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        while let Some(token) = lexer.token() {
            let Token::Keyword(op) = token else {
                if let Some(o) = lexer.from_token(token, false, 0) {
                    operands.push(o);
                }
                continue;
            };
            self.operations += 1;
            if self.operations > MAX_OPERATIONS {
                return;
            }
            let num = |i: usize| operands.get(i).and_then(Object::number).unwrap_or(0.0);
            let n = operands.len();
            let last = |k: usize| -> f64 {
                if n >= k {
                    operands[n - k].number().unwrap_or(0.0)
                } else {
                    0.0
                }
            };
            match op {
                b"q" => {
                    if stack.len() < 256 {
                        stack.push(state.clone());
                    }
                }
                b"Q" => {
                    if let Some(s) = stack.pop() {
                        state = s;
                    }
                }
                b"cm" if n >= 6 => {
                    let m = [last(6), last(5), last(4), last(3), last(2), last(1)];
                    state.ctm = multiply(&m, &state.ctm);
                }
                b"BT" => {
                    tm = IDENTITY;
                    tlm = IDENTITY;
                }
                b"Tf" if n >= 2 => {
                    if let Some(name) = operands[n - 2].name() {
                        let name = name.to_vec();
                        let (font, index) = self.font(resources, &name);
                        state.font = Some(font);
                        state.font_index = index;
                    }
                    state.size = last(1);
                }
                b"Tc" => state.char_space = last(1),
                b"Tw" => state.word_space = last(1),
                b"Tz" => state.scale = last(1) / 100.0,
                b"TL" => state.leading = last(1),
                b"Ts" => state.rise = last(1),
                b"Tr" => state.render = last(1) as i64,
                b"Td" | b"TD" if n >= 2 => {
                    let (tx, ty) = (last(2), last(1));
                    if op == b"TD" {
                        state.leading = -ty;
                    }
                    tlm = multiply(&[1.0, 0.0, 0.0, 1.0, tx, ty], &tlm);
                    tm = tlm;
                }
                b"Tm" if n >= 6 => {
                    tlm = [last(6), last(5), last(4), last(3), last(2), last(1)];
                    tm = tlm;
                }
                b"T*" => {
                    tlm = multiply(&[1.0, 0.0, 0.0, 1.0, 0.0, -state.leading], &tlm);
                    tm = tlm;
                }
                b"Tj" => {
                    if let Some(s) = operands.last().and_then(Object::string) {
                        let s = s.to_vec();
                        self.show(&s, &state, &mut tm);
                    }
                }
                b"'" | b"\"" => {
                    if op == b"\"" && n >= 3 {
                        state.word_space = last(3);
                        state.char_space = last(2);
                    }
                    tlm = multiply(&[1.0, 0.0, 0.0, 1.0, 0.0, -state.leading], &tlm);
                    tm = tlm;
                    if let Some(s) = operands.last().and_then(Object::string) {
                        let s = s.to_vec();
                        self.show(&s, &state, &mut tm);
                    }
                }
                b"TJ" => {
                    if let Some(items) = operands.last().and_then(Object::array) {
                        let items = items.to_vec();
                        for item in items {
                            match item {
                                Object::String(s) => self.show(&s, &state, &mut tm),
                                other => {
                                    let adjust = other.number().unwrap_or(0.0);
                                    let tx = -adjust / 1000.0 * state.size * state.scale;
                                    tm = multiply(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], &tm);
                                }
                            }
                        }
                    }
                }
                b"g" => state.fill_white = is_white_color(&[num(0)], false),
                b"G" => state.stroke_white = is_white_color(&[num(0)], false),
                b"rg" => state.fill_white = is_white_color(&[num(0), num(1), num(2)], false),
                b"RG" => state.stroke_white = is_white_color(&[num(0), num(1), num(2)], false),
                b"k" => state.fill_white = is_white_color(&[num(0), num(1), num(2), num(3)], true),
                b"K" => {
                    state.stroke_white = is_white_color(&[num(0), num(1), num(2), num(3)], true);
                }
                b"sc" | b"scn" => {
                    let values: Vec<f64> = operands.iter().filter_map(Object::number).collect();
                    state.fill_white = values.len() != 4 && is_white_color(&values, false)
                        || values.len() == 4 && is_white_color(&values, true);
                }
                b"SC" | b"SCN" => {
                    let values: Vec<f64> = operands.iter().filter_map(Object::number).collect();
                    state.stroke_white = values.len() != 4 && is_white_color(&values, false)
                        || values.len() == 4 && is_white_color(&values, true);
                }
                b"cs" => state.fill_white = false,
                b"CS" => state.stroke_white = false,
                b"m" if n >= 2 => {
                    current = apply(&state.ctm, last(2), last(1));
                    start = current;
                }
                b"l" if n >= 2 => {
                    let p = apply(&state.ctm, last(2), last(1));
                    path.push((current.0, current.1, p.0, p.1));
                    current = p;
                }
                b"c" if n >= 6 => current = apply(&state.ctm, last(2), last(1)),
                b"v" | b"y" if n >= 4 => current = apply(&state.ctm, last(2), last(1)),
                b"h" => {
                    path.push((current.0, current.1, start.0, start.1));
                    current = start;
                }
                b"re" if n >= 4 => {
                    let (x, y, w, h) = (last(4), last(3), last(2), last(1));
                    let corners = [
                        apply(&state.ctm, x, y),
                        apply(&state.ctm, x + w, y),
                        apply(&state.ctm, x + w, y + h),
                        apply(&state.ctm, x, y + h),
                    ];
                    for i in 0..4 {
                        let (a, b) = (corners[i], corners[(i + 1) % 4]);
                        path.push((a.0, a.1, b.0, b.1));
                    }
                    current = corners[0];
                    start = current;
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" => {
                    let stroke = matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
                    let fill = !matches!(op, b"S" | b"s");
                    if (stroke && !state.stroke_white) || (fill && !state.fill_white) {
                        if matches!(op, b"s" | b"b" | b"b*") {
                            path.push((current.0, current.1, start.0, start.1));
                        }
                        for &(x0, y0, x1, y1) in &path {
                            self.segment(x0, y0, x1, y1);
                        }
                    }
                    path.clear();
                }
                b"n" => path.clear(),
                b"Do" => {
                    if let Some(name) = operands.last().and_then(Object::name) {
                        let name = name.to_vec();
                        self.xobject(resources, &name, &state, depth);
                    }
                }
                b"BI" => {
                    Self::skip_inline_image(&mut lexer);
                    self.image(&state.ctm);
                }
                _ => {}
            }
            operands.clear();
        }
    }

    fn show(&mut self, bytes: &[u8], state: &State, tm: &mut Matrix) {
        let font = state
            .font
            .clone()
            .unwrap_or_else(|| Rc::new(Font::fallback()));
        let invisible = state.render == 3 || state.render == 7;
        for glyph in font.decode(bytes) {
            let text_matrix = multiply(tm, &state.ctm);
            let trm = multiply(
                &[
                    state.size * state.scale,
                    0.0,
                    0.0,
                    state.size,
                    0.0,
                    state.rise,
                ],
                &text_matrix,
            );
            let size = trm[2].hypot(trm[3]);
            let visible = glyph.width * state.size * state.scale;
            let (dx, dy) = (visible * text_matrix[0], visible * text_matrix[1]);
            if size > 0.01 && !glyph.text.is_empty() {
                self.out.glyphs.push(PlacedGlyph {
                    text: glyph.text.clone(),
                    x: trm[4],
                    y: trm[5],
                    dx,
                    dy,
                    size,
                    angle: trm[1].atan2(trm[0]),
                    font: state.font_index,
                    invisible,
                });
            }
            let tx = (glyph.width * state.size
                + state.char_space
                + if glyph.space { state.word_space } else { 0.0 })
                * state.scale;
            *tm = multiply(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], tm);
        }
    }

    fn segment(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
        if dx < 0.5 && dy >= 1.0 {
            let x = f64::midpoint(x0, x1);
            self.out.segments.push(Segment {
                x0: x,
                y0: y0.min(y1),
                x1: x,
                y1: y0.max(y1),
            });
        } else if dy < 0.5 && dx >= 1.0 {
            let y = f64::midpoint(y0, y1);
            self.out.segments.push(Segment {
                x0: x0.min(x1),
                y0: y,
                x1: x0.max(x1),
                y1: y,
            });
        }
    }

    fn image(&mut self, ctm: &Matrix) {
        let corners = [
            apply(ctm, 0.0, 0.0),
            apply(ctm, 1.0, 0.0),
            apply(ctm, 0.0, 1.0),
            apply(ctm, 1.0, 1.0),
        ];
        let xs = corners.iter().map(|c| c.0);
        let ys = corners.iter().map(|c| c.1);
        self.out.images.push([
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        ]);
    }

    fn xobject(&mut self, resources: &Dict, name: &[u8], state: &State, depth: usize) {
        let xobjects = self.doc.entry(resources, "XObject");
        let Some(reference) = xobjects.dict().and_then(|d| d.get(name)).cloned() else {
            return;
        };
        let Object::Stream(stream) = self.doc.resolve(&reference) else {
            return;
        };
        match self.doc.entry(&stream.dict, "Subtype").name() {
            Some(b"Image") => self.image(&state.ctm),
            Some(b"Form") => {
                if depth >= 12 {
                    return;
                }
                let number = match reference {
                    Object::Ref(n, _) => Some(n),
                    _ => None,
                };
                if let Some(n) = number
                    && !self.forms.insert(n)
                {
                    return;
                }
                let matrix = self.doc.entry(&stream.dict, "Matrix").numbers();
                let mut inner = state.clone();
                if matrix.len() == 6 {
                    let m = [
                        matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
                    ];
                    inner.ctm = multiply(&m, &state.ctm);
                }
                let own = self.doc.entry(&stream.dict, "Resources");
                let inner_resources = own.dict().cloned().unwrap_or_else(|| resources.clone());
                let data = self.doc.decode(&stream);
                self.content(&data, &inner_resources, inner, depth + 1);
                if let Some(n) = number {
                    self.forms.remove(&n);
                }
            }
            _ => {}
        }
    }

    /// Skips `… ID <data> EI` of an inline image.
    fn skip_inline_image(lexer: &mut Lexer<'_>) {
        while let Some(token) = lexer.token() {
            if token == Token::Keyword(b"ID") {
                break;
            }
        }
        let data = lexer.data();
        let mut i = lexer.pos + 1;
        while i + 2 <= data.len() {
            if data[i] == b'E'
                && data[i + 1] == b'I'
                && is_white(data[i - 1])
                && data.get(i + 2).is_none_or(|&c| is_white(c))
            {
                lexer.pos = i + 2;
                return;
            }
            i += 1;
        }
        lexer.pos = data.len();
    }
}
