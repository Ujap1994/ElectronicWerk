//! Benchmarks for encoding and decoding synthetic designs.
//!
//! Run with `cargo bench -p ew-format`. The L size takes a while; filter
//! with `cargo bench -p ew-format -- "/S"` for a quick run.

use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use ew_format::codec;
use ew_model::document::Document;
use ew_synth::{DesignSize, generate};

fn board(size: DesignSize) -> Document {
    let project = generate(size, ew_synth::DEFAULT_SEED);
    project
        .documents
        .into_values()
        .find(|document| matches!(document, Document::Pcb(_)))
        .unwrap_or_else(|| Document::new(ew_model::DocumentKind::Pcb, ew_model::Id::NIL, "x"))
}

fn bench_board_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("board");
    group.sample_size(10);
    for size in [DesignSize::S, DesignSize::M, DesignSize::L] {
        let document = board(size);
        let text = codec::encode_document(&document).unwrap_or_default();
        group.bench_function(format!("encode/{size}"), |b| {
            b.iter(|| codec::encode_document(black_box(&document)));
        });
        group.bench_function(format!("decode/{size}"), |b| {
            b.iter(|| {
                codec::decode_document(Path::new("board"), document.kind(), black_box(&text))
            });
        });
    }
    group.finish();
}

fn bench_generate(c: &mut Criterion) {
    c.bench_function("generate/M", |b| {
        b.iter(|| generate(black_box(DesignSize::M), 1));
    });
}

criterion_group!(benches, bench_board_io, bench_generate);
criterion_main!(benches);
