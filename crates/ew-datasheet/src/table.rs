//! Tables of a page: from the drawn rules (cells of a grid, merged where a
//! rule is missing) or, without rules, from the alignment of the text under
//! a header line.

use crate::text::{Line, Page, Rect, Word};

/// A table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Text: the words in reading order, lines joined by a space.
    pub text: String,
    /// Bounds.
    pub bbox: Rect,
    /// First column.
    pub column: usize,
    /// Number of columns it spans.
    pub span: usize,
    /// Words in bold.
    pub bold: bool,
}

/// A row of cells, left to right.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Cells.
    pub cells: Vec<Cell>,
    /// Bounds.
    pub bbox: Rect,
}

impl Row {
    /// The text of the cell covering `column`, or an empty string.
    #[must_use]
    pub fn text(&self, column: usize) -> &str {
        self.cells
            .iter()
            .find(|c| c.column <= column && column < c.column + c.span)
            .map_or("", |c| c.text.as_str())
    }

    /// The cell covering `column`.
    #[must_use]
    pub fn cell(&self, column: usize) -> Option<&Cell> {
        self.cells
            .iter()
            .find(|c| c.column <= column && column < c.column + c.span)
    }
}

/// A table of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Page number (1-based).
    pub page: usize,
    /// Bounds.
    pub bbox: Rect,
    /// Number of columns.
    pub columns: usize,
    /// Rows, top to bottom.
    pub rows: Vec<Row>,
    /// Made from the rules (else from the text alignment).
    pub ruled: bool,
}

const TOLERANCE: f64 = 2.0;

fn center(r: &Rect) -> (f64, f64) {
    (f64::midpoint(r[0], r[2]), f64::midpoint(r[1], r[3]))
}

fn inside(point: (f64, f64), r: &Rect) -> bool {
    point.0 >= r[0] && point.0 <= r[2] && point.1 >= r[1] && point.1 <= r[3]
}

/// Merges sorted values closer than the tolerance.
fn clusters(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for v in values {
        match out.last_mut() {
            Some(last) if v - *last < TOLERANCE * 1.5 => *last = f64::midpoint(*last, v),
            _ => out.push(v),
        }
    }
    out
}

/// Text of words in reading order.
fn join(words: &[&Word]) -> String {
    let mut sorted: Vec<&&Word> = words.iter().collect();
    sorted.sort_by(|a, b| {
        let same_line = (a.bbox[3] - b.bbox[3]).abs() < 0.5 * a.size.min(b.size);
        if same_line {
            a.bbox[0].total_cmp(&b.bbox[0])
        } else {
            a.bbox[3].total_cmp(&b.bbox[3])
        }
    });
    let mut out = String::new();
    for w in sorted {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&w.text);
    }
    out
}

struct Groups(Vec<usize>);

impl Groups {
    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while self.0[root] != root {
            root = self.0[root];
        }
        let mut k = i;
        while self.0[k] != root {
            let next = self.0[k];
            self.0[k] = root;
            k = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0[rb] = ra;
        }
    }
}

/// Whether a rule covers most of the span `[from, to]` at `at`.
fn covered(rules: &[[f64; 4]], horizontal: bool, at: f64, from: f64, to: f64) -> bool {
    let length = to - from;
    let mut total = 0.0;
    for r in rules {
        let (fixed, start, end) = if horizontal {
            (r[1], r[0], r[2])
        } else {
            (r[0], r[1], r[3])
        };
        if (fixed - at).abs() <= TOLERANCE {
            total += (end.min(to) - start.max(from)).max(0.0);
        }
    }
    total >= 0.6 * length
}

/// Tables drawn with rules.
#[must_use]
#[allow(clippy::too_many_lines, reason = "grouping, grid and text in one pass")]
pub fn ruled(page: &Page) -> Vec<Table> {
    let horizontal: Vec<[f64; 4]> = page
        .rules
        .iter()
        .filter(|r| (r[1] - r[3]).abs() < 0.5)
        .copied()
        .collect();
    let vertical: Vec<[f64; 4]> = page
        .rules
        .iter()
        .filter(|r| (r[0] - r[2]).abs() < 0.5)
        .copied()
        .collect();
    // Connected groups of rules.
    let all: Vec<[f64; 4]> = horizontal.iter().chain(vertical.iter()).copied().collect();
    let n = all.len();
    if n == 0 || n > 4000 {
        return Vec::new();
    }
    let mut groups = Groups((0..n).collect());
    let touches = |a: &[f64; 4], b: &[f64; 4]| {
        a[0] <= b[2] + TOLERANCE
            && b[0] <= a[2] + TOLERANCE
            && a[1] <= b[3] + TOLERANCE
            && b[1] <= a[3] + TOLERANCE
    };
    for i in 0..n {
        for j in i + 1..n {
            if touches(&all[i], &all[j]) {
                groups.union(i, j);
            }
        }
    }
    let mut members: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for i in 0..n {
        let root = groups.find(i);
        members.entry(root).or_default().push(i);
    }
    let words: Vec<&Word> = page
        .lines
        .iter()
        .filter(|l| l.angle == 0)
        .flat_map(|l| l.words.iter())
        .collect();
    let mut out = Vec::new();
    for list in members.values() {
        let rules: Vec<[f64; 4]> = list.iter().map(|&i| all[i]).collect();
        let hs: Vec<[f64; 4]> = rules
            .iter()
            .filter(|r| (r[1] - r[3]).abs() < 0.5)
            .copied()
            .collect();
        let vs: Vec<[f64; 4]> = rules
            .iter()
            .filter(|r| (r[0] - r[2]).abs() < 0.5)
            .copied()
            .collect();
        if hs.len() < 2 {
            continue;
        }
        let bbox = [
            rules.iter().map(|r| r[0]).fold(f64::INFINITY, f64::min),
            rules.iter().map(|r| r[1]).fold(f64::INFINITY, f64::min),
            rules.iter().map(|r| r[2]).fold(f64::NEG_INFINITY, f64::max),
            rules.iter().map(|r| r[3]).fold(f64::NEG_INFINITY, f64::max),
        ];
        if bbox[2] - bbox[0] < 30.0 || bbox[3] - bbox[1] < 8.0 {
            continue;
        }
        let inner: Vec<&Word> = words
            .iter()
            .copied()
            .filter(|w| inside(center(&w.bbox), &bbox))
            .collect();
        if inner.len() < 2 {
            continue;
        }
        let ys = clusters(hs.iter().map(|r| r[1]).collect());
        let mut xs = clusters(vs.iter().map(|r| r[0]).collect());
        let mut vertical_rules = vs.clone();
        if xs.len() < 3 {
            // Only row rules: columns from the text alignment.
            xs = text_columns(&inner, bbox);
            vertical_rules = xs.iter().map(|x| [*x, bbox[1], *x, bbox[3]]).collect();
        }
        if xs.first().is_none_or(|x| *x > bbox[0] + TOLERANCE) {
            xs.insert(0, bbox[0]);
            vertical_rules.push([bbox[0], bbox[1], bbox[0], bbox[3]]);
        }
        if xs.last().is_none_or(|x| *x < bbox[2] - TOLERANCE) {
            xs.push(bbox[2]);
            vertical_rules.push([bbox[2], bbox[1], bbox[2], bbox[3]]);
        }
        let mut ys = ys;
        if ys.first().is_none_or(|y| *y > bbox[1] + TOLERANCE) {
            ys.insert(0, bbox[1]);
        }
        if ys.last().is_none_or(|y| *y < bbox[3] - TOLERANCE) {
            ys.push(bbox[3]);
        }
        if let Some(table) = grid(page.number, &xs, &ys, &hs, &vertical_rules, &inner, bbox) {
            out.push(table);
        }
    }
    out.sort_by(|a, b| a.bbox[1].total_cmp(&b.bbox[1]));
    out
}

/// Column boundaries from gaps that no word crosses.
fn text_columns(words: &[&Word], bbox: Rect) -> Vec<f64> {
    // One cell per point of the table's width: a damaged coordinate must not
    // make that billions of cells (pages are at most 14,400 pt wide).
    let span = bbox[2] - bbox[0];
    if !span.is_finite() || span > 14_400.0 {
        return Vec::new();
    }
    let width = span.ceil().max(1.0) as usize;
    let mut cover = vec![0u32; width + 1];
    for w in words {
        let a = ((w.bbox[0] - bbox[0]).max(0.0)) as usize;
        let b = ((w.bbox[2] - bbox[0]).max(0.0) as usize).min(width);
        for c in cover.iter_mut().take(b + 1).skip(a) {
            *c += 1;
        }
    }
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let first_word = words
        .iter()
        .map(|w| w.bbox[0] - bbox[0])
        .fold(f64::INFINITY, f64::min)
        .max(0.0) as usize;
    let last_word = words
        .iter()
        .map(|w| w.bbox[2] - bbox[0])
        .fold(0.0, f64::max) as usize;
    for (i, &c) in cover.iter().enumerate() {
        if c == 0 && i > first_word && i < last_word {
            start.get_or_insert(i);
        } else if let Some(s) = start.take()
            && i - s >= 6
        {
            out.push(bbox[0] + (s + i) as f64 / 2.0);
        }
    }
    out
}

/// Builds the cells of a grid and fills in the words.
fn grid(
    page: usize,
    xs: &[f64],
    ys: &[f64],
    hs: &[[f64; 4]],
    vs: &[[f64; 4]],
    words: &[&Word],
    bbox: Rect,
) -> Option<Table> {
    let (nx, ny) = (xs.len().checked_sub(1)?, ys.len().checked_sub(1)?);
    if nx == 0 || ny == 0 || nx * ny > 20_000 {
        return None;
    }
    let index = |i: usize, j: usize| j * nx + i;
    let mut groups = Groups((0..nx * ny).collect());
    for j in 0..ny {
        for i in 0..nx {
            // Right neighbour: joined unless a vertical rule separates.
            if i + 1 < nx && !covered(vs, false, xs[i + 1], ys[j], ys[j + 1]) {
                groups.union(index(i, j), index(i + 1, j));
            }
            // Lower neighbour: joined unless a horizontal rule separates.
            if j + 1 < ny && !covered(hs, true, ys[j + 1], xs[i], xs[i + 1]) {
                groups.union(index(i, j), index(i, j + 1));
            }
        }
    }
    // Cells: the bounds of each group.
    let mut cells: std::collections::BTreeMap<usize, (usize, usize, usize, usize)> =
        std::collections::BTreeMap::new();
    for j in 0..ny {
        for i in 0..nx {
            let root = groups.find(index(i, j));
            let e = cells.entry(root).or_insert((i, j, i, j));
            e.0 = e.0.min(i);
            e.1 = e.1.min(j);
            e.2 = e.2.max(i);
            e.3 = e.3.max(j);
        }
    }
    let mut rows: Vec<Row> = (0..ny)
        .map(|j| Row {
            cells: Vec::new(),
            bbox: [xs[0], ys[j], xs[nx], ys[j + 1]],
        })
        .collect();
    for (i0, j0, i1, j1) in cells.values() {
        let r = [xs[*i0], ys[*j0], xs[*i1 + 1], ys[*j1 + 1]];
        let inner: Vec<&Word> = words
            .iter()
            .copied()
            .filter(|w| inside(center(&w.bbox), &r))
            .collect();
        rows[*j0].cells.push(Cell {
            text: join(&inner),
            bbox: r,
            column: *i0,
            span: i1 - i0 + 1,
            bold: !inner.is_empty() && inner.iter().all(|w| w.bold),
        });
    }
    for row in &mut rows {
        row.cells.sort_by_key(|c| c.column);
    }
    // Rows without any text (spacing bands) are dropped.
    rows.retain(|r| r.cells.iter().any(|c| !c.text.is_empty()));
    if rows.is_empty() {
        return None;
    }
    Some(Table {
        page,
        bbox,
        columns: nx,
        rows,
        ruled: true,
    })
}

/// A table without rules below a header line: columns start at the header
/// words (joined where they are close), rows are the lines until a gap or
/// a heading. Lines without text in the `key` column continue the row
/// above.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "columns, rows and continuations in one pass"
)]
pub fn under_header(page: &Page, header: usize, key: usize) -> Option<Table> {
    let lines: Vec<&Line> = page.lines.iter().filter(|l| l.angle == 0).collect();
    let head = lines.get(header)?;
    // Column starts from header words, joining words of one header cell.
    let mut starts: Vec<(f64, f64)> = Vec::new();
    for w in &head.words {
        match starts.last_mut() {
            Some(last) if w.bbox[0] - last.1 < 0.6 * w.size => last.1 = w.bbox[2],
            _ => starts.push((w.bbox[0], w.bbox[2])),
        }
    }
    if starts.len() < 2 {
        return None;
    }
    let columns = starts.len();
    let column_of = |x: f64| -> usize {
        let mut column = 0;
        for (i, (s, _)) in starts.iter().enumerate() {
            if x >= s - 8.0 {
                column = i;
            }
        }
        column
    };
    let mut rows: Vec<Row> = Vec::new();
    let mut previous_bottom = head.bbox[3];
    let header_cells: Vec<Cell> = starts
        .iter()
        .enumerate()
        .map(|(i, (s, e))| {
            let words: Vec<&Word> = head
                .words
                .iter()
                .filter(|w| w.bbox[0] >= s - 0.5 && w.bbox[2] <= e + 0.5)
                .collect();
            Cell {
                text: join(&words),
                bbox: [*s, head.bbox[1], *e, head.bbox[3]],
                column: i,
                span: 1,
                bold: words.iter().all(|w| w.bold),
            }
        })
        .collect();
    rows.push(Row {
        cells: header_cells,
        bbox: head.bbox,
    });
    for line in lines.iter().skip(header + 1) {
        let gap = line.bbox[1] - previous_bottom;
        if gap > 2.5 * line.size || line.size > head.size * 1.3 {
            break;
        }
        previous_bottom = line.bbox[3];
        let mut cells: Vec<Cell> = Vec::new();
        for w in &line.words {
            let column = column_of(w.bbox[0]);
            match cells.iter_mut().find(|c| c.column == column) {
                Some(c) => {
                    c.text.push(' ');
                    c.text.push_str(&w.text);
                    c.bbox[2] = c.bbox[2].max(w.bbox[2]);
                    c.bold &= w.bold;
                }
                None => cells.push(Cell {
                    text: w.text.clone(),
                    bbox: w.bbox,
                    column,
                    span: 1,
                    bold: w.bold,
                }),
            }
        }
        let continues = !cells.iter().any(|c| c.column == key) && rows.len() > 1;
        if continues {
            let Some(row) = rows.last_mut() else {
                break;
            };
            for c in cells {
                match row.cells.iter_mut().find(|r| r.column == c.column) {
                    Some(r) => {
                        r.text.push(' ');
                        r.text.push_str(&c.text);
                        r.bbox[3] = r.bbox[3].max(c.bbox[3]);
                    }
                    None => row.cells.push(c),
                }
            }
            row.bbox[3] = line.bbox[3];
            row.cells.sort_by_key(|c| c.column);
        } else {
            cells.sort_by_key(|c| c.column);
            rows.push(Row {
                cells,
                bbox: line.bbox,
            });
        }
    }
    if rows.len() < 2 {
        return None;
    }
    let bbox = [
        rows.iter().map(|r| r.bbox[0]).fold(f64::INFINITY, f64::min),
        rows[0].bbox[1],
        rows.iter()
            .map(|r| r.bbox[2])
            .fold(f64::NEG_INFINITY, f64::max),
        rows.last().map_or(0.0, |r| r.bbox[3]),
    ];
    Some(Table {
        page: page.number,
        bbox,
        columns,
        rows,
        ruled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x: f64, y: f64) -> Word {
        Word {
            text: text.into(),
            bbox: [x, y - 8.0, x + 6.0 * text.len() as f64, y + 2.0],
            size: 10.0,
            bold: false,
        }
    }

    fn line(words: Vec<Word>) -> Line {
        let bbox = [
            words[0].bbox[0],
            words[0].bbox[1],
            words.last().map_or(0.0, |w| w.bbox[2]),
            words[0].bbox[3],
        ];
        Line {
            text: words
                .iter()
                .map(|w| w.text.clone())
                .collect::<Vec<_>>()
                .join(" "),
            bbox,
            size: 10.0,
            angle: 0,
            words,
        }
    }

    #[test]
    fn reads_a_grid_with_a_merged_header() {
        // Columns at 0, 40, 80, 200; the header "PIN" spans the first two.
        let rules = vec![
            [0.0, 0.0, 200.0, 0.0],
            [0.0, 15.0, 80.0, 15.0], // only below "PIN"
            [0.0, 30.0, 200.0, 30.0],
            [0.0, 45.0, 200.0, 45.0],
            [0.0, 0.0, 0.0, 45.0],
            [40.0, 15.0, 40.0, 45.0],
            [80.0, 0.0, 80.0, 45.0],
            [200.0, 0.0, 200.0, 45.0],
        ];
        let page = Page {
            number: 1,
            width: 300.0,
            height: 300.0,
            lines: vec![
                line(vec![
                    word("PIN", 30.0, 11.0),
                    word("DESCRIPTION", 100.0, 20.0),
                ]),
                line(vec![word("NO.", 5.0, 26.0), word("NAME", 45.0, 26.0)]),
                line(vec![
                    word("1", 5.0, 41.0),
                    word("VCC", 45.0, 41.0),
                    word("Supply", 90.0, 41.0),
                ]),
            ],
            rules,
            scanned: false,
        };
        let tables = ruled(&page);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.columns, 3);
        let texts: Vec<Vec<String>> = t
            .rows
            .iter()
            .map(|r| r.cells.iter().map(|c| c.text.clone()).collect())
            .collect();
        assert_eq!(
            texts,
            vec![
                vec!["PIN".to_owned(), "DESCRIPTION".to_owned()],
                vec!["NO.".to_owned(), "NAME".to_owned()],
                vec!["1".to_owned(), "VCC".to_owned(), "Supply".to_owned()],
            ]
        );
        assert_eq!(t.rows[0].cells[0].span, 2);
        assert_eq!(t.rows[2].text(2), "Supply");
    }

    #[test]
    fn reads_a_table_without_rules_under_its_header() {
        let page = Page {
            number: 2,
            width: 300.0,
            height: 300.0,
            lines: vec![
                line(vec![
                    word("Pin", 10.0, 10.0),
                    word("Name", 60.0, 10.0),
                    word("Function", 120.0, 10.0),
                ]),
                line(vec![
                    word("1", 10.0, 24.0),
                    word("OUT", 60.0, 24.0),
                    word("Output", 120.0, 24.0),
                ]),
                line(vec![word("of", 120.0, 36.0), word("channel", 140.0, 36.0)]),
                line(vec![
                    word("2", 10.0, 48.0),
                    word("GND", 60.0, 48.0),
                    word("Ground", 120.0, 48.0),
                ]),
            ],
            rules: Vec::new(),
            scanned: false,
        };
        let t = under_header(&page, 0, 0).unwrap_or_else(|| panic!("no table"));
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.rows[1].text(2), "Output of channel");
        assert_eq!(t.rows[2].text(1), "GND");
    }
}
