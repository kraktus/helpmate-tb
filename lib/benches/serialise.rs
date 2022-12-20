use std::io::{Cursor, IoSliceMut, Read, Write};

use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    BinWriterExt,
};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use deku::{bitvec::BitView, ctx::Limit, prelude::*, DekuRead, DekuWrite};
use helpmate_tb::{handle_symetry, Indexer, Material, NaiveIndexer, SideToMove, Table};
use retroboard::RetroBoard;
use serde::{Deserialize, Serialize};

#[derive(DekuRead, BinRead, BinWrite, DekuWrite, Serialize, Deserialize)]
struct Test {
    pub a: u64,
    pub b: u64,
    pub c: u64,
}

impl Test {
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
        .map(|i| Test {
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
        .map(|i| Test {
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
            <Vec<Test> as DekuRead<Limit<_, _>>>::read(
                custom.view_bits(),
                Limit::new_count(10_0000),
            )
        })
    });
    let mut binrw = Cursor::new(custom.clone());
    binrw.set_position(0);
    group.bench_function("custom", move |b| {
        b.iter_batched(
            || custom.clone(),
            |mut custom_input| {
                for _ in 0..10_0000 {
                    Test::from_bytes_custom(&mut custom_input.as_slice());
                }
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("binrw", move |b| {
        b.iter_batched(
            || binrw.clone(),
            |mut binrw_input| {
                for i in 0..10_0000 {
                    Test::read_le(&mut binrw_input).expect(&format!("{i}"));
                }
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("bincode", |b| {
        b.iter(|| bincode::deserialize::<Vec<Test>>(&bincode).unwrap())
    });
    group.finish();
}

criterion_group!(benches, bench_serialise, bench_deserialise);
criterion_main!(benches);
