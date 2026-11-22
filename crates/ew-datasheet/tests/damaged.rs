//! Damaged datasheets that once made the reader hang.

use std::time::{Duration, Instant};

#[test]
fn a_table_with_a_damaged_coordinate_is_read_quickly() {
    // The synthetic EW7021 datasheet with one coordinate turned huge: the
    // column search once tried a cell for every point of its width.
    let data = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/damaged-table.pdf"),
    )
    .unwrap();
    let started = Instant::now();
    if let Ok(sheet) = ew_datasheet::read(data) {
        let _ = ew_datasheet::recognize::recognize(&sheet);
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}
