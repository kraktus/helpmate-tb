use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use helpmate_tb::{Indexer, Material, NaiveIndexer, Table};
use retroboard::RetroBoard;

pub fn bench_indexers(c: &mut Criterion) {
    let fens = [
        "8/8/8/8/8/8/8/KNBk4 w - - 0 1",
        "8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1",
        "8/8/2k5/8/4N3/2K2B2/8/8 w - - 0 1",
        "8/8/8/8/8/8/N7/KBk5 b - - 0 1",
        "8/8/8/8/8/k7/B7/KN6 b - - 0 1",
        "1R6/8/8/3K4/8/8/2R5/7k w - - 0 1",
        "7k/2R5/8/8/3K4/8/8/1R6 w - - 0 1",
        "5Q2/8/8/Q7/8/2k5/8/K7 b - -",
        "2Q5/8/8/7Q/8/5k2/8/7K b - -",
        "8/8/2N5/3B4/8/2K2k2/8/8 b - - 0 1",
        "8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1",
        "8/8/8/8/8/8/2Q5/k1K5 w - -",
        "8/8/8/8/8/8/2q5/K1k5 b - - 0 1",
        "8/8/8/8/8/2K5/Q7/2k5 w - -",
        "8/8/8/8/8/k1K5/8/1Q6 w - - 0 1",
        "8/8/8/8/N7/k7/8/2KB4 w - - 0 1",
        "8/8/8/8/B7/K7/8/2kN4 w - - 0 1",
    ];
    let rboards_and_syzygy = fens.map(|fen| {
        let rboard = RetroBoard::new_no_pockets(fen).unwrap();
        let mat = Material::from_board(rboard.board());
        (rboard, Table::from(mat))
    });
    let mut group = c.benchmark_group("Indexer");
    for (i, (rboard, syzygy)) in rboards_and_syzygy.into_iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("Naive", i), &rboard, |b, rboard_ref| {
            b.iter(|| NaiveIndexer.encode(rboard_ref))
        });
        group.bench_with_input(BenchmarkId::new("Syzygy", i), &rboard, |b, rboard_ref| {
            b.iter(|| syzygy.encode(rboard_ref))
        });
    }
}

criterion_group!(benches, bench_indexers);
criterion_main!(benches);
