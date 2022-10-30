pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};

use env_logger::{Builder, Target};
use log::LevelFilter;

use retroboard::shakmaty::Color;
use std::collections::HashMap;
use std::fs::File;

use log::debug;

use clap::Parser;

#[cfg(dhat)]
use dhat::{Dhat, DhatAlloc};

#[cfg(dhat)]
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
    #[cfg(dhat)]
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
        root_material.descendants_recursive(false)
    } else {
        vec![]
    };
    materials.push(root_material);
    for mat in materials {
        gen_one_material(mat)
    }
}

fn gen_one_material(mat: Material) {
    for winner in Color::ALL {
        // white first, most interesting
        let common = TableBaseBuilder::build(mat.clone(), winner);
        let mut encoder = EncoderDecoder::new(
            File::create(format!(
                "table/{:?}",
                MaterialWinner::new(&common.material, common.winner)
            ))
            .unwrap(),
        );
        encoder
            .compress(&common.all_pos)
            .expect("Compression failed");
        stats(&common)
    }
}

fn stats(common: &Common) {
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut unkown = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();
    let mut undefined_outcome: usize = 0;

    for by_color_outcome in common.all_pos.iter() {
        if &UNDEFINED_OUTCOME_BYCOLOR == by_color_outcome {
            undefined_outcome += 2;
            continue;
        };
        for color in Color::ALL {
            let outcome = by_color_outcome.get_by_color(color).outcome();
            distrib.insert(outcome, *distrib.get(&outcome).unwrap_or(&0) + 1);
            match outcome {
                Outcome::Draw => draw += 1,
                Outcome::Win(_) => win += 1,
                Outcome::Lose(_) => lose += 1,
                Outcome::Undefined => undefined_outcome += 1,
                Outcome::Unknown => unkown += 1,
            }
        }
    }
    debug!(
        "From {:?} perspective, win: {win:?}, draw: {draw:?}, lost: {lose:?}, unkown: {unkown:?}",
        common.winner
    );
    debug!(
        "Index density = {:?}%",
        (common.all_pos.len() * 2 - undefined_outcome) * 100 / (common.all_pos.len() * 2)
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