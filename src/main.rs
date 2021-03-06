mod compression;
mod encoding;
mod file_handler;
mod generation;
mod indexer;
mod indexer_syzygy;
mod material;
mod outcome;

pub use crate::file_handler::TableBase;
pub use crate::outcome::{Outcome, Outcomes, OutcomesSlice, Report, UNDEFINED_OUTCOME_BYCOLOR};
pub use compression::EncoderDecoder;
pub use encoding::get_info_table;
pub use generation::{Generator, SideToMove, SideToMoveGetter};
pub use indexer::{index, index_unchecked, restore_from_index};
pub use indexer_syzygy::{Pieces, Table, A1_H8_DIAG, A8_H1_DIAG};
pub use material::Material;

use env_logger::{Builder, Target};
use log::LevelFilter;

use std::collections::HashMap;
use std::fs::File;

use log::debug;

use clap::Parser;

use dhat::{Dhat, DhatAlloc};

#[global_allocator]
static ALLOCATOR: DhatAlloc = DhatAlloc;
// 3 pieces before using index At t-gmax: 19,080,095 bytes (100%) in 47 blocks (100%), avg size 405,959.47 bytes
// 4 pieces before using index At t-gmax: 610,457,858 bytes (100%) in 199 blocks (100%), avg size 3,067,627.43 bytes

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opt {
    #[clap(short, long, value_parser, help = "example \"KQvK\"")]
    material: String,
    #[clap(short, long, parse(from_flag))]
    recursive: bool,
    #[clap(short, long, action = clap::ArgAction::Count, default_value_t = 2)]
    verbose: u8,
}

fn main() {
    let _dhat = Dhat::start_heap_profiling();
    let args = Opt::parse();
    let mut builder = Builder::new();
    builder
        .filter(
            None,
            match args.verbose {
                0 => LevelFilter::Error,
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            },
        )
        .default_format()
        .target(Target::Stdout)
        .init();
    let root_material = Material::from_str(&args.material).expect("Valid material config");
    let mut materials = if args.recursive {
        root_material.descendants_not_draw_recursive()
    } else {
        vec![]
    };
    materials.push(root_material);
    for mat in materials {
        gen_one_material(mat)
    }
}

fn gen_one_material(mat: Material) {
    let mut gen = Generator::new(mat);
    let mut q = gen.generate_positions();
    debug!("nb pos {:?}", gen.all_pos.len());
    debug!("counter {:?}", gen.counter);
    debug!(
        "nb {:?} mates {:?}",
        gen.winner,
        q.winning_pos_to_process.len()
    );
    debug!(
        "nb {:?} mates {:?}",
        !gen.winner,
        q.losing_pos_to_process.len()
    );
    // need to process FIRST winning positions, then losing ones.
    gen.process_positions(&mut q.winning_pos_to_process);
    gen.process_positions(&mut q.losing_pos_to_process);
    let mut encoder =
        EncoderDecoder::new(File::create(format!("table/{:?}", &gen.material)).unwrap());
    encoder.compress(&gen.all_pos).expect("Compression failed");
    stats(&gen)
}

fn stats(gen: &Generator) {
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();
    let mut undefined_outcome: usize = 0;

    //println!("{:?}", gen.all_pos);
    for by_color_outcome in gen.all_pos.iter() {
        if &UNDEFINED_OUTCOME_BYCOLOR == by_color_outcome {
            undefined_outcome += 2;
            continue;
        };
        for color in shakmaty::Color::ALL {
            let outcome = by_color_outcome.get_by_color(color).outcome();
            distrib.insert(outcome, *distrib.get(&outcome).unwrap_or(&0) + 1);
            match outcome {
                Outcome::Draw => draw += 1,
                Outcome::Win(_) => win += 1,
                Outcome::Lose(_) => lose += 1,
                Outcome::Undefined => undefined_outcome += 1,
            }
        }
    }
    debug!(
        "From {:?} perspective, win: {win:?}, draw: {draw:?}, lost: {lose:?}",
        gen.winner
    );
    debug!(
        "Index density = {:?}%",
        (gen.all_pos.len() * 2 - undefined_outcome) * 100 / (gen.all_pos.len() * 2)
    );
    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Win(i)) {
            debug!("Win({}), {:?}", i, nb_win);
        }
    }

    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Lose(i)) {
            debug!("Lose({}), {:?}", i, nb_win);
        }
    }
}
