//! Criterion benchmarks for the qalam-text pipeline.
//!
//! Measures throughput of `normalize`, `tokenize`, and `word_frequencies`
//! against the performance targets in `DESIGN.md` §5.5 (>=50k tokens/sec
//! single-threaded). Run with `cargo bench -p qalam-text`.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use qalam_text::{freq, tokenize, unicode};

/// A representative paragraph of MSA prose, including alef variants and tatweel
/// so the normalizer does real work.
const PARA: &str = "اللغة العربية من أقدم اللغات الحية في العالم، ولها تاريخ \
طويل وحضارة عريقة. يكتب الكاتب الكتاب في المكتبة، ويدرس الطلاب في المدرسة \
دروسهم بجدٍّ واجتهاد. إنّ المعرفة نورٌ، والجهل ظلامٌ، والكــــتاب خير جليس.";

fn sample() -> String {
    PARA.repeat(200)
}

fn bench_normalize(c: &mut Criterion) {
    let text = sample();
    let mut group = c.benchmark_group("normalize");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("paragraph_x200", |b| {
        b.iter(|| unicode::normalize(black_box(&text)))
    });
    group.finish();
}

fn bench_tokenize(c: &mut Criterion) {
    let text = sample();
    let mut group = c.benchmark_group("tokenize");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("paragraph_x200", |b| {
        b.iter(|| tokenize::tokenize(black_box(&text)))
    });
    group.finish();
}

fn bench_freq(c: &mut Criterion) {
    let text = sample();
    let tokens = tokenize::tokenize(&text);
    let mut group = c.benchmark_group("freq");
    group.throughput(Throughput::Elements(tokens.len() as u64));
    group.bench_function("paragraph_x200", |b| {
        b.iter(|| freq::word_frequencies(black_box(&tokens)))
    });
    group.finish();
}

criterion_group!(benches, bench_normalize, bench_tokenize, bench_freq);
criterion_main!(benches);
