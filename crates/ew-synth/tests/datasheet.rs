//! The synthetic datasheets read back and recognised.

use ew_datasheet::draft::{Family, PinType};
use ew_datasheet::read;
use ew_datasheet::recognize::recognize;
use ew_synth::datasheet::{ew4410, ew7021};

/// Writes the files for a look with the `dump` example:
/// `EW_DATASHEET_OUT=dir cargo test -p ew-synth --test datasheet -- --ignored`.
#[test]
#[ignore = "writes files for a manual look"]
fn write_files() {
    if let Some(dir) = std::env::var_os("EW_DATASHEET_OUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(dir.join("ew7021.pdf"), ew7021()).unwrap_or_default();
        std::fs::write(dir.join("ew4410.pdf"), ew4410()).unwrap_or_default();
    }
}

#[test]
fn recognises_the_ew7021() {
    let sheet = read(ew7021()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(sheet.pages.len(), 6);
    let draft = recognize(&sheet);
    assert_eq!(draft.name, "EW7021");
    assert_eq!(draft.manufacturer, "Example Semiconductors");
    assert_eq!(draft.description, "Dual Low-Power Comparator");
    let names: Vec<&str> = draft.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["DFN-8", "MSOP-8", "SOIC-8"]);
    let pins: Vec<&str> = draft.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        pins,
        vec![
            "OUT1", "IN1-", "IN1+", "GND", "IN2+", "IN2-", "OUT2", "VCC", "EP"
        ]
    );
    let vcc = &draft.pins[7];
    assert_eq!(vcc.kind, PinType::PowerIn);
    assert_eq!(vcc.numbers.get("SOIC-8"), Some(&vec!["8".to_owned()]));
    assert_eq!(vcc.numbers.get("DFN-8"), Some(&vec!["8".to_owned()]));
    let ep = &draft.pins[8];
    assert_eq!(ep.numbers.get("DFN-8"), Some(&vec!["9".to_owned()]));
    assert!(!ep.numbers.contains_key("SOIC-8"));
    assert_eq!(draft.pins[0].kind, PinType::Output);
    assert_eq!(draft.pins[1].kind, PinType::Input);
    // SOIC-8 dimensions from the ruled table, in millimetres.
    let soic = draft
        .packages
        .iter()
        .find(|p| p.name == "SOIC-8")
        .unwrap_or_else(|| panic!("no SOIC"));
    assert_eq!(soic.family, Family::Sop);
    assert_eq!(soic.pins, 8);
    let value = |p: &ew_datasheet::draft::PackageDraft, k: &str| {
        p.dimensions
            .get(k)
            .and_then(ew_datasheet::draft::Dimension::value)
    };
    assert_eq!(value(soic, "D"), Some(4.9));
    assert_eq!(value(soic, "E1"), Some(3.9));
    assert_eq!(value(soic, "e"), Some(1.27));
    assert_eq!(soic.dimensions.get("b").and_then(|d| d.min), Some(0.31));
    assert!(
        soic.dimensions
            .get("D")
            .and_then(|d| d.source.as_ref())
            .is_some_and(|s| s.page == 4)
    );
    // DFN-8 from the aligned text table, with its exposed pad.
    let dfn = draft
        .packages
        .iter()
        .find(|p| p.name == "DFN-8")
        .unwrap_or_else(|| panic!("no DFN"));
    assert_eq!(dfn.family, Family::Dfn);
    assert!(dfn.exposed_pad);
    assert_eq!(value(dfn, "D2"), Some(2.38));
    assert_eq!(value(dfn, "e"), Some(0.65));
    assert!(
        dfn.dimensions
            .get("E2")
            .and_then(|d| d.source.as_ref())
            .is_some_and(|s| s.page == 5)
    );
    // MSOP-8: standard values, no source.
    let msop = draft
        .packages
        .iter()
        .find(|p| p.name == "MSOP-8")
        .unwrap_or_else(|| panic!("no MSOP"));
    assert_eq!(value(msop, "e"), Some(0.65));
    assert!(msop.dimensions.values().all(|d| d.source.is_none()));
}

#[test]
fn recognises_the_ew4410() {
    let sheet = read(ew4410()).unwrap_or_else(|e| panic!("{e}"));
    assert!(sheet.encrypted);
    let draft = recognize(&sheet);
    assert_eq!(draft.name, "EW4410");
    let names: Vec<&str> = draft.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["LQFP-32", "QFN-32"]);
    assert_eq!(
        draft
            .pins
            .iter()
            .filter(|p| p.name.starts_with("PA"))
            .count(),
        8
    );
    assert_eq!(
        draft
            .pins
            .iter()
            .filter(|p| p.name.starts_with("PB"))
            .count(),
        8
    );
    let gnd = draft
        .pins
        .iter()
        .find(|p| p.name == "GND")
        .unwrap_or_else(|| panic!("no GND"));
    assert_eq!(
        gnd.numbers.get("QFN-32"),
        Some(&vec!["16".to_owned(), "33".to_owned()])
    );
    let int = draft
        .pins
        .iter()
        .find(|p| p.name == "INT")
        .unwrap_or_else(|| panic!("no INT"));
    assert_eq!(int.kind, PinType::OpenCollector);
    let nc = draft
        .pins
        .iter()
        .find(|p| p.name == "NC")
        .unwrap_or_else(|| panic!("no NC"));
    assert_eq!(nc.numbers.get("LQFP-32").map(Vec::len), Some(8));
    let qfn = draft
        .packages
        .iter()
        .find(|p| p.name == "QFN-32")
        .unwrap_or_else(|| panic!("no QFN"));
    assert_eq!(
        qfn.dimensions
            .get("E")
            .and_then(ew_datasheet::draft::Dimension::value),
        Some(5.0)
    );
}

#[test]
fn generates_parts_that_pass_the_library_checks() {
    use ew_datasheet::part::{PartOptions, generate};
    use ew_libedit::check::{Severity, check_footprint, check_symbol};
    for (pdf, packages) in [
        (ew7021(), vec!["SOIC-8", "MSOP-8", "DFN-8"]),
        (ew4410(), vec!["QFN-32", "LQFP-32"]),
    ] {
        let draft = recognize(&read(pdf).unwrap_or_else(|e| panic!("{e}")));
        for package in packages {
            let part = generate(&draft, package, &PartOptions::default())
                .unwrap_or_else(|e| panic!("{package}: {e}"));
            assert!(part.warnings.is_empty(), "{package}: {:?}", part.warnings);
            let errors: Vec<String> = check_footprint(&part.footprint)
                .into_iter()
                .chain(check_symbol(&part.symbol))
                .filter(|f| f.severity == Severity::Error)
                .map(|f| f.message)
                .collect();
            assert!(errors.is_empty(), "{package}: {errors:?}");
            let pins: usize = part.symbol.units.iter().map(|u| u.pins.len()).sum();
            assert_eq!(pins, part.footprint.pads.len(), "{package}");
            assert_eq!(
                part.component.footprint.as_deref(),
                Some(part.footprint.name.as_str())
            );
        }
    }
}
