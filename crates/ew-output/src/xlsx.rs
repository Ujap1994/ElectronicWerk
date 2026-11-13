//! A minimal Excel workbook (Office Open XML) writer: one sheet with a
//! bold, frozen header row, an auto filter and column widths that fit the
//! content.

use std::fmt::Write as _;

use crate::OutputFile;
use crate::archive::zip;
use crate::bom::{Cell, escape_xml};

/// The column name of a 0-based index (`A`, …, `Z`, `AA`, …).
fn column_name(mut index: usize) -> String {
    let mut name = Vec::new();
    loop {
        name.push(b'A' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    name.reverse();
    String::from_utf8(name).unwrap_or_default()
}

fn sheet_xml(headers: &[String], rows: &[Vec<Cell>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(cell.text().chars().count());
            }
        }
    }
    let last = column_name(headers.len().saturating_sub(1));
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
    );
    out.push_str(
        "<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" \
         activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>",
    );
    out.push_str("<cols>");
    for (i, width) in widths.iter().enumerate() {
        let _ = write!(
            out,
            "<col min=\"{n}\" max=\"{n}\" width=\"{}\" customWidth=\"1\"/>",
            (*width).clamp(4, 60) + 2,
            n = i + 1
        );
    }
    out.push_str("</cols><sheetData>");
    let line = |out: &mut String, number: usize, cells: Vec<Cell>, style: u8| {
        let _ = write!(out, "<row r=\"{number}\">");
        for (i, cell) in cells.iter().enumerate() {
            let reference = format!("{}{number}", column_name(i));
            let s = if style > 0 {
                format!(" s=\"{style}\"")
            } else {
                String::new()
            };
            match cell {
                Cell::Number(v) => {
                    let _ = write!(out, "<c r=\"{reference}\"{s}><v>{v}</v></c>");
                }
                Cell::Text(t) => {
                    let _ = write!(
                        out,
                        "<c r=\"{reference}\"{s} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        escape_xml(t)
                    );
                }
            }
        }
        out.push_str("</row>");
    };
    line(
        &mut out,
        1,
        headers.iter().map(|h| Cell::Text(h.clone())).collect(),
        1,
    );
    for (i, row) in rows.iter().enumerate() {
        line(&mut out, i + 2, row.clone(), 0);
    }
    let _ = write!(
        out,
        "</sheetData><autoFilter ref=\"A1:{last}{}\"/></worksheet>",
        rows.len() + 1
    );
    out
}

/// A workbook with one sheet.
#[must_use]
pub fn workbook(sheet: &str, headers: &[String], rows: &[Vec<Cell>]) -> Vec<u8> {
    let sheet_name: String = escape_xml(
        &sheet
            .chars()
            .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
            .take(31)
            .collect::<String>(),
    );
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
        <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
        <Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
        <Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\
        </Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
        </Relationships>";
    let workbook = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
        xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
        <sheets><sheet name=\"{sheet_name}\" sheetId=\"1\" r:id=\"rId1\"/></sheets>\
        <definedNames><definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"0\" hidden=\"1\">'{sheet_name}'!$A$1:${}${}</definedName></definedNames>\
        </workbook>",
        column_name(headers.len().saturating_sub(1)),
        rows.len() + 1
    );
    let workbook_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
        <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
        </Relationships>";
    let styles = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
        <fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font>\
        <font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\
        <fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>\
        <borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
        <cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
        <cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
        <xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/></cellXfs>\
        </styleSheet>";
    zip(&[
        OutputFile::text("[Content_Types].xml", content_types.to_owned()),
        OutputFile::text("_rels/.rels", rels.to_owned()),
        OutputFile::text("xl/workbook.xml", workbook),
        OutputFile::text("xl/_rels/workbook.xml.rels", workbook_rels.to_owned()),
        OutputFile::text("xl/styles.xml", styles.to_owned()),
        OutputFile::text("xl/worksheets/sheet1.xml", sheet_xml(headers, rows)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read as _};

    #[test]
    fn column_names() {
        assert_eq!(column_name(0), "A");
        assert_eq!(column_name(25), "Z");
        assert_eq!(column_name(26), "AA");
        assert_eq!(column_name(27), "AB");
        assert_eq!(column_name(701), "ZZ");
        assert_eq!(column_name(702), "AAA");
    }

    #[test]
    fn writes_a_workbook() {
        let bytes = workbook(
            "BOM: demo",
            &["Qty".into(), "Value".into()],
            &[vec![Cell::Number(2.0), Cell::Text("10µF <x>".into())]],
        );
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains("<c r=\"A2\"><v>2</v></c>"), "{sheet}");
        assert!(sheet.contains("10µF &lt;x&gt;"));
        assert!(sheet.contains("<autoFilter ref=\"A1:B2\"/>"));
        let mut book = String::new();
        archive
            .by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut book)
            .unwrap();
        assert!(book.contains("name=\"BOM demo\""));
    }
}
