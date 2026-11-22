//! Robustness of every reader against damaged files: thousands of mutated
//! copies of real files (bits flipped, bytes replaced, ranges deleted,
//! duplicated or cut off, huge or odd numbers put in) must give a result or
//! an error — never a panic and never a hang.
//!
//! The normal test run tries 60 mutations per file; set `EW_FUZZ_ROUNDS`
//! for a longer run (`EW_FUZZ_ROUNDS=5000 cargo test --release -p ew-core
//! --test robustness`) and `EW_FUZZ_ONLY` to try only some readers
//! (`EW_FUZZ_ONLY=gerber,pdf`). Inputs that fail are saved under
//! `target/fuzz-failures/` to reproduce them.

// The test reports its progress and hangs on stderr.
#![allow(clippy::print_stderr)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ODD_NUMBERS: &[&str] = &[
    "-1",
    "0",
    "-0",
    "1e308",
    "-1e308",
    "1e-400",
    "NaN",
    "inf",
    "99999999999999999999",
    "4294967296",
    "-2147483649",
    "1.5",
    "0x10",
    "",
];
const ODD_BYTES: &[u8] = b"()[]{}<>\"'\\/ \n\t0-9e.,;:=#&\x00\xff";

/// A small, reproducible random number generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            usize::try_from(self.next() % (n as u64)).unwrap_or(0)
        }
    }
}

/// Damaged inputs stay below this size (repeated lines would grow without
/// end; a real file of gigabytes is a test of its own).
const MAX_INPUT: usize = 4 << 20;

/// One to eight random edits of the data.
fn mutate(rng: &mut Rng, data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for _ in 0..=rng.below(8) {
        if out.len() > MAX_INPUT {
            out.truncate(MAX_INPUT);
        }
        let len = out.len();
        let at = rng.below(len.max(1));
        match rng.below(8) {
            0 if len > 0 => out[at] ^= 1 << rng.below(8),
            1 if len > 0 => out[at] = ODD_BYTES[rng.below(ODD_BYTES.len())],
            2 => {
                let end = (at + 1 + rng.below(64)).min(len);
                out.drain(at.min(end)..end);
            }
            3 => {
                let end = (at + 1 + rng.below(256)).min(len);
                let copy = out[at.min(end)..end].to_vec();
                let to = rng.below(out.len() + 1);
                out.splice(to..to, copy);
            }
            4 => out.truncate(at),
            5 => {
                // A number replaced by an odd one.
                let digits = out
                    .iter()
                    .enumerate()
                    .skip(at)
                    .find(|(_, b)| b.is_ascii_digit());
                if let Some((start, _)) = digits {
                    let end = out[start..]
                        .iter()
                        .position(|b| !(b.is_ascii_digit() || *b == b'.'))
                        .map_or(out.len(), |n| start + n);
                    let odd = ODD_NUMBERS[rng.below(ODD_NUMBERS.len())].as_bytes();
                    out.splice(start..end, odd.iter().copied());
                }
            }
            6 if len < MAX_INPUT / 4 => {
                // A line repeated many times (deep nesting, long lists).
                let start = out[..at]
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map_or(0, |n| n + 1);
                let end = out[at..]
                    .iter()
                    .position(|b| *b == b'\n')
                    .map_or(len, |n| at + n + 1);
                let line = out[start..end].to_vec();
                let copies = line.repeat(1 + rng.below(200).min(MAX_INPUT / line.len().max(1)));
                out.splice(end..end, copies);
            }
            _ => {
                let odd = ODD_NUMBERS[rng.below(ODD_NUMBERS.len())].as_bytes();
                out.splice(at..at, odd.iter().copied());
            }
        }
    }
    out
}

fn rounds() -> usize {
    std::env::var("EW_FUZZ_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> Vec<u8> {
    std::fs::read(repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The panic message of the last panic on this thread.
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

/// Runs `reader` on mutations of each input; returns the failures.
fn fuzz(name: &str, inputs: &[(&str, Vec<u8>)], reader: &dyn Fn(&[u8])) -> Vec<String> {
    let mut failures = Vec::new();
    // `EW_FUZZ_ONLY=gerber,pdf` tries only these readers.
    if let Ok(only) = std::env::var("EW_FUZZ_ONLY")
        && !only.split(',').any(|n| n.trim() == name)
    {
        return failures;
    }
    let dir = repo().join("target/fuzz-failures");
    let current = Arc::new(Mutex::new((String::new(), Vec::new())));
    let progress = Arc::new(AtomicU64::new(0));
    let (done, finished) = std::sync::mpsc::channel::<()>();
    // A watchdog: a reader that hangs is saved and reported.
    let watchdog = {
        let current = Arc::clone(&current);
        let progress = Arc::clone(&progress);
        let dir = dir.clone();
        std::thread::spawn(move || {
            let mut last = u64::MAX;
            loop {
                if finished.recv_timeout(Duration::from_secs(20)).is_ok() {
                    return;
                }
                let now = progress.load(Ordering::SeqCst);
                if now == last {
                    let (label, data) = current.lock().map(|c| c.clone()).unwrap_or_default();
                    let _ = std::fs::create_dir_all(&dir);
                    let path = dir.join(format!("{label}.hang"));
                    let _ = std::fs::write(&path, data);
                    eprintln!("HANG: {label} (saved as {})", path.display());
                    std::process::exit(1);
                }
                last = now;
            }
        })
    };
    for (file, data) in inputs {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (file.len() as u64 + 1));
        for round in 0..rounds() {
            let input = mutate(&mut rng, data);
            let label = format!("{name}-{}-{round}", file.replace(['/', '.'], "_"));
            if let Ok(mut c) = current.lock() {
                *c = (label.clone(), input.clone());
            }
            let started = Instant::now();
            let outcome = catch_unwind(AssertUnwindSafe(|| reader(&input)));
            let took = started.elapsed();
            progress.fetch_add(1, Ordering::SeqCst);
            if outcome.is_err() || took > Duration::from_secs(5) {
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("{label}.bin"));
                let _ = std::fs::write(&path, &input);
                let message = LAST_PANIC.lock().map(|m| m.clone()).unwrap_or_default();
                failures.push(if outcome.is_err() {
                    format!("{label}: panic: {message} ({})", path.display())
                } else {
                    format!("{label}: took {took:?} ({})", path.display())
                });
            }
        }
    }
    let _ = done.send(());
    let _ = watchdog.join();
    eprintln!(
        "{name}: {} damaged inputs read",
        progress.load(Ordering::SeqCst)
    );
    failures
}

fn quiet_panics() {
    std::panic::set_hook(Box::new(|info| {
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = info.to_string();
        }
    }));
}

fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one block per reader")]
fn every_reader_survives_damaged_files() {
    quiet_panics();
    let mut failures = Vec::new();

    // KiCad S-expressions and whole projects.
    let kicad = [
        "crates/ew-kicad/tests/fixtures/blinky/blinky.kicad_pcb",
        "crates/ew-kicad/tests/fixtures/blinky/blinky.kicad_sch",
        "crates/ew-kicad/tests/fixtures/blinky/Blinky.kicad_sym",
    ];
    let kicad_inputs: Vec<(&str, Vec<u8>)> = kicad.iter().map(|p| (*p, read(p))).collect();
    failures.extend(fuzz("sexpr", &kicad_inputs, &|data| {
        let _ = ew_kicad::sexpr::parse(&text(data));
    }));
    let project = repo().join("crates/ew-kicad/tests/fixtures/blinky");
    failures.extend(fuzz("kicad-import", &kicad_inputs, &|data| {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        copy_dir(&project, dir.path());
        // The damaged file replaces its original (the first one that matches).
        for name in ["blinky.kicad_pcb", "blinky.kicad_sch", "Blinky.kicad_sym"] {
            if data.len() > 10 && name.ends_with(&ext_of(data)) {
                let _ = std::fs::write(dir.path().join(name), data);
            }
        }
        let _ = ew_kicad::import_project(&dir.path().join("blinky.kicad_pro"), None);
    }));

    // Eagle.
    let eagle_dir = repo().join("crates/ew-eagle/tests/fixtures/blinky");
    let eagle = [
        "crates/ew-eagle/tests/fixtures/blinky/blinky.brd",
        "crates/ew-eagle/tests/fixtures/blinky/blinky.sch",
    ];
    let eagle_inputs: Vec<(&str, Vec<u8>)> = eagle.iter().map(|p| (*p, read(p))).collect();
    failures.extend(fuzz("eagle-import", &eagle_inputs, &|data| {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        copy_dir(&eagle_dir, dir.path());
        let name = if text(data).contains("<board") {
            "blinky.brd"
        } else {
            "blinky.sch"
        };
        let _ = std::fs::write(dir.path().join(name), data);
        let _ = ew_eagle::import_project(&dir.path().join("blinky.brd"), None);
    }));

    // Gerber and Excellon.
    let gerber_dir = "crates/ew-output/tests/fixtures/blinky-kicad";
    let gerber_files: Vec<String> = std::fs::read_dir(repo().join(gerber_dir))
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|e| format!("{gerber_dir}/{}", e.file_name().to_string_lossy()))
                .filter(|p| !p.to_ascii_lowercase().ends_with(".d356"))
                .collect()
        })
        .unwrap_or_default();
    let gerber_inputs: Vec<(&str, Vec<u8>)> =
        gerber_files.iter().map(|p| (p.as_str(), read(p))).collect();
    failures.extend(fuzz("gerber", &gerber_inputs, &|data| {
        let _ = ew_output::read::read(&text(data));
    }));

    // Netlists.
    let netlists = [
        "crates/ew-kicad/tests/fixtures/blinky/blinky.net",
        "crates/ew-kicad/tests/fixtures/blinky/blinky-netlist.xml",
        "crates/ew-kicad/tests/fixtures/blinky/blinky-netlist.csv",
    ];
    let netlist_inputs: Vec<(&str, Vec<u8>)> = netlists.iter().map(|p| (*p, read(p))).collect();
    failures.extend(fuzz("netlist", &netlist_inputs, &|data| {
        let _ = ew_kicad::netlist::parse(&text(data));
    }));

    // Specctra sessions.
    let ses = [(
        "crates/ew-core/tests/fixtures/freerouting/blinky.ses",
        read("crates/ew-core/tests/fixtures/freerouting/blinky.ses"),
    )];
    failures.extend(fuzz("specctra", &ses, &|data| {
        let _ = ew_pcb::specctra::read_ses(&text(data));
    }));

    // DXF, written from the imported board.
    let imported = ew_kicad::import_project(&project.join("blinky.kicad_pro"), None)
        .unwrap_or_else(|e| panic!("{e}"));
    let pcb = imported.project.documents.values().find_map(|d| match d {
        ew_model::document::Document::Pcb(pcb) => Some(pcb.clone()),
        _ => None,
    });
    if let Some(pcb) = pcb {
        let options = ew_output::dxf::DxfOptions {
            layers: Vec::new(),
            inch: false,
            separate: false,
        };
        let files = ew_output::dxf::dxf_files(&pcb, &options, "blinky");
        let dxf_inputs: Vec<(&str, Vec<u8>)> = files
            .iter()
            .take(2)
            .map(|f| ("dxf", f.data.clone()))
            .collect();
        failures.extend(fuzz("dxf", &dxf_inputs, &|data| {
            let _ = ew_pcb::dxf::read(&text(data), None);
        }));
    }

    // STEP.
    let steps = [
        "crates/ew-board3d/tests/fixtures/blinky-assembly.step",
        "crates/ew-library/tests/fixtures/kicad/3dmodels/Demo.3dshapes/SOIC-8.step",
    ];
    let step_inputs: Vec<(&str, Vec<u8>)> = steps.iter().map(|p| (*p, read(p))).collect();
    failures.extend(fuzz("step", &step_inputs, &|data| {
        let text = text(data);
        let _ = ew_board3d::step::part21::parse(&text);
        let _ = ew_board3d::step::tessellate::planar_mesh(&text);
    }));

    // PDF datasheets.
    let pdf_inputs = [
        ("ew7021.pdf", ew_synth::datasheet::ew7021()),
        ("ew4410.pdf", ew_synth::datasheet::ew4410()),
    ];
    failures.extend(fuzz("pdf", &pdf_inputs, &|data| {
        if let Ok(sheet) = ew_datasheet::read(data.to_vec()) {
            let _ = ew_datasheet::recognize::recognize(&sheet);
        }
    }));

    // ElectronicWerk's own files.
    let synthetic = ew_synth::generate(ew_synth::DesignSize::S, 7);
    let own: Vec<(&str, Vec<u8>)> = synthetic
        .documents
        .values()
        .take(3)
        .filter_map(|d| {
            ew_format::codec::encode_document(d)
                .ok()
                .map(|t| ("document", t.into_bytes()))
        })
        .collect();
    let kinds: Vec<ew_model::document::DocumentKind> = synthetic
        .documents
        .values()
        .take(3)
        .map(ew_model::document::Document::kind)
        .collect();
    failures.extend(fuzz("own-format", &own, &|data| {
        for kind in &kinds {
            let _ = ew_format::codec::decode_document(Path::new("x"), *kind, &text(data));
        }
    }));

    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The extension a damaged file most likely has (by its first token).
fn ext_of(data: &[u8]) -> String {
    let head = text(&data[..data.len().min(64)]);
    if head.contains("kicad_pcb") {
        ".kicad_pcb".to_owned()
    } else if head.contains("kicad_sch") {
        ".kicad_sch".to_owned()
    } else if head.contains("kicad_symbol_lib") {
        ".kicad_sym".to_owned()
    } else {
        "\u{0}".to_owned()
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            let _ = std::fs::create_dir_all(&target);
            copy_dir(&entry.path(), &target);
        } else {
            let _ = std::fs::copy(entry.path(), &target);
        }
    }
}
