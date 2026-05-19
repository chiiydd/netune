//! Criterion benchmarks for netune-api LRC parsing.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Generate a simple LRC string with the given number of lines.
fn generate_lrc(line_count: usize) -> String {
    let mut lrc = String::with_capacity(line_count * 40);
    for i in 0..line_count {
        let minutes = i / 60;
        let seconds = i % 60;
        let millis = (i * 37) % 100;
        lrc.push_str(&format!(
            "[{:02}:{:02}.{:02}]Line {} of lyrics content here\n",
            minutes, seconds, millis, i
        ));
    }
    lrc
}

fn bench_parse_lrc_small(c: &mut Criterion) {
    let lrc = generate_lrc(10);

    c.bench_function("parse_lrc_10_lines", |b| {
        b.iter(|| {
            netune_api::client::parse_lrc(black_box(&lrc));
        })
    });
}

fn bench_parse_lrc_large(c: &mut Criterion) {
    let lrc = generate_lrc(1000);

    c.bench_function("parse_lrc_1000_lines", |b| {
        b.iter(|| {
            netune_api::client::parse_lrc(black_box(&lrc));
        })
    });
}

fn bench_parse_lrc_with_translations(c: &mut Criterion) {
    let original = generate_lrc(50);
    let translated = (0..50)
        .map(|i| {
            let minutes = i / 60;
            let seconds = i % 60;
            let millis = (i * 37) % 100;
            format!(
                "[{:02}:{:02}.{:02}]翻译歌词第{}行\n",
                minutes, seconds, millis, i
            )
        })
        .collect::<String>();

    c.bench_function("parse_lrc_original_50_lines", |b| {
        b.iter(|| {
            netune_api::client::parse_lrc(black_box(&original));
        })
    });

    c.bench_function("parse_lrc_translated_50_lines", |b| {
        b.iter(|| {
            netune_api::client::parse_lrc(black_box(&translated));
        })
    });
}

criterion_group!(
    lrc_benches,
    bench_parse_lrc_small,
    bench_parse_lrc_large,
    bench_parse_lrc_with_translations,
);
criterion_main!(lrc_benches);
