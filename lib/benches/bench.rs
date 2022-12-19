use std::io::{Cursor, IoSliceMut, Read, Write};

use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    BinWriterExt,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use deku::{bitvec::BitView, ctx::Limit, prelude::*, DekuRead, DekuWrite};
use helpmate_tb::{handle_symetry, Indexer, Material, NaiveIndexer, SideToMove, Table};
use retroboard::RetroBoard;
use serde::{Deserialize, Serialize};

fn bench_indexers(c: &mut Criterion) {
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
    {
        let mut group = c.benchmark_group("CheckedIndexer");
        for (i, (rboard, syzygy)) in rboards_and_syzygy.into_iter().enumerate() {
            group.bench_with_input(BenchmarkId::new("Naive", i), &rboard, |b, rboard_ref| {
                b.iter(|| NaiveIndexer.encode(rboard_ref))
            });
            group.bench_with_input(BenchmarkId::new("Syzygy", i), &rboard, |b, rboard_ref| {
                b.iter(|| syzygy.encode(rboard_ref))
            });
        }
        group.finish()
    }

    let checked_boards_and_syzygy = fens.map(|fen| {
        let rboard = RetroBoard::new_no_pockets(fen).unwrap();
        let mat = Material::from_board(rboard.board());
        let (board_check, is_black_stronger) = handle_symetry(rboard.board());
        (
            (board_check, rboard.side_to_move() ^ is_black_stronger),
            Table::from(mat),
        )
    });

    let mut group = c.benchmark_group("UncheckedIndexer");

    for (i, (side_to_move, syzygy)) in checked_boards_and_syzygy.into_iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("Naive", i),
            &side_to_move,
            |b, side_to_move_ref| b.iter(|| NaiveIndexer.encode_unchecked(side_to_move_ref)),
        );
        group.bench_with_input(
            BenchmarkId::new("Syzygy", i),
            &side_to_move,
            |b, side_to_move_ref| b.iter(|| syzygy.encode_unchecked(side_to_move_ref)),
        );
    }
    group.finish()
}

#[derive(DekuRead, BinRead, BinWrite, DekuWrite, Serialize, Deserialize)]
struct TestCompression {
    pub a: u64,
    pub b: u64,
    pub c: u64,
}

impl TestCompression {
    fn to_bytes_custom<T: Write>(&self, writer: &mut T) {
        writer.write(&self.a.to_ne_bytes()).unwrap();
        writer.write(&self.b.to_ne_bytes()).unwrap();
        writer.write(&self.c.to_ne_bytes()).unwrap();
    }

    fn from_bytes_custom<T: Read>(reader: &mut T) -> Self {
        let (mut a, mut b, mut c) = ([0; 8], [0; 8], [0; 8]);
        let mut buf = [
            IoSliceMut::new(&mut a),
            IoSliceMut::new(&mut b),
            IoSliceMut::new(&mut c),
        ];
        reader.read_vectored(&mut buf).unwrap();
        Self {
            a: u64::from_ne_bytes(a),
            b: u64::from_ne_bytes(b),
            c: u64::from_ne_bytes(c),
        }
    }
}

fn bench_serialise(c: &mut Criterion) {
    let input: Vec<_> = (0..10_0000)
        .map(|i| TestCompression {
            a: i,
            b: i + 1,
            c: i + 2,
        })
        .collect();
    let mut group = c.benchmark_group("Serialize");
    group.bench_function("deku", |b| {
        b.iter(|| {
            let _: Vec<u8> = input
                .iter()
                .flat_map(|x| x.to_bytes().unwrap().into_iter())
                .collect();
        })
    });
    group.bench_function("binrw", |b| {
        b.iter(|| {
            let mut buf = Cursor::new(Vec::new());
            buf.write_le(&input).unwrap();
        })
    });
    group.bench_function("custom", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            for i in input.iter() {
                i.to_bytes_custom(&mut buf)
            }
        })
    });
    group.bench_function("bincode", |b| {
        b.iter(|| bincode::serialize(&input).unwrap())
    });
    group.finish();
    let mut buf = Cursor::new(Vec::new());
    buf.write_le(&input).unwrap();
    let binrw = buf.into_inner();
    assert_eq!(
        binrw,
        input
            .iter()
            .flat_map(|x| x.to_bytes().unwrap().into_iter())
            .collect::<Vec<_>>()
    );
    let mut custom = Vec::new();
    for i in input.iter() {
        i.to_bytes_custom(&mut custom)
    }
    assert_eq!(binrw, custom);
    // bincode is different
    // assert_eq!(custom, bincode::serialize(&input).unwrap());
}

fn bench_deserialise(c: &mut Criterion) {
    let input: Vec<_> = (0..10_0000)
        .map(|i| TestCompression {
            a: i,
            b: i + 1,
            c: i + 2,
        })
        .collect();
    let mut custom = Vec::new();
    for i in input.iter() {
        i.to_bytes_custom(&mut custom)
    }
    let bincode = bincode::serialize(&input).unwrap();

    let mut group = c.benchmark_group("Deserialize");
    group.bench_function("deku", |b| {
        b.iter(|| {
            <Vec<TestCompression> as DekuRead<Limit<_, _>>>::read(
                custom.view_bits(),
                Limit::new_count(10_0000),
            )
        })
    });
    let mut binrw = Cursor::new(custom.clone());
    group.bench_function("custom", |b| {
        b.iter(|| {
            for _ in 0..10_0000 {
                TestCompression::from_bytes_custom(&mut custom.as_slice());
            }
        })
    });
    //     group.bench_function("binrw", |b| {
    //     b.iter(|| {
    //         for i in 0..10_0000 {
    //             TestCompression::read_le(&mut binrw).expect(&format!("{i}"));
    //         }
    //     })
    // });
    group.bench_function("bincode", |b| {
        b.iter(|| bincode::deserialize::<Vec<TestCompression>>(&bincode).unwrap())
    });
    group.finish();
}

criterion_group!(benches, bench_deserialise);
criterion_main!(benches);
