use env_logger::{Builder, Target};
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, FileHandler, IndexWithTurn, Outcomes};
use log::{info, LevelFilter};
use std::{collections::HashMap, path::Path, str::FromStr};

use retroboard::shakmaty::Color;

use clap::{ArgAction, Parser};

#[cfg(feature = "dhat")]
#[global_allocator]
static DHAT_ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Opt {
    #[arg(help = "example \"KQvK\", use special value 'all' to search across all positions")]
    material: String,
    #[arg(short, long, help = "Color of the expected winner", default_value_t = Color::White)]
    winner: Color,
    #[arg(long,
        value_parser = Outcome::from_str,
        help = "If draw is selected, only non-stalemate ones will be returned"
    )]
    outcome: Option<Outcome>,
    #[arg(short, long, action = ArgAction::Count, default_value_t = 2)]
    verbose: u8,
    #[arg(long, action = ArgAction::SetFalse, default_value_t = false)]
    exclude_summary: bool,
}

fn main() {
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::new_heap();
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
        .target(Target::Stdout);

    builder.init();
    if args.material == "all" {
        let entries = Path::new("./table")
            .read_dir()
            .expect("read_dir call failed");
        for entry_res in entries {
            let mut mat_str = entry_res.unwrap().file_name().into_string().unwrap();
            let winner = match mat_str.pop() {
                Some('b') => Color::Black,
                Some('w') => Color::White,
                _ => panic!("Only black and white can be winners"),
            };
            let mat = Material::from_str(&mat_str).expect("Valid material config");
            let mat_win = MaterialWinner::new(&mat, winner);
            stats_one_mat(mat_win, &args);
        }
    } else {
        let mat = Material::from_str(&args.material).expect("Valid material config");
        let mat_win = MaterialWinner::new(&mat, args.winner);
        stats_one_mat(mat_win, &args);
    }
}

fn stats_one_mat(mat_win: MaterialWinner, args: &Opt) {
    info!(
        "Generating {:?} with winner: {}",
        mat_win.material, mat_win.winner
    );
    let file_handler: FileHandler = FileHandler::new(&mat_win);
    if !args.exclude_summary {
        stats(mat_win, &file_handler, args.outcome)
    }
}

fn stats(mat_win: MaterialWinner, file_handler: &FileHandler, searched_outcome: Option<Outcome>) {
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut unkown = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();
    let mut undefined_outcome: usize = 0;

    for (idx, by_color_outcome) in file_handler.outcomes.iter().enumerate() {
        for turn in Color::ALL {
            let outcome = by_color_outcome.get_by_color(turn);
            if Some(outcome) == searched_outcome {
                info!(
                    "Macthing {outcome:?}, position {:?}",
                    file_handler.indexer.restore(
                        mat_win.material,
                        IndexWithTurn {
                            idx: idx as u64,
                            turn
                        }
                    )
                )
            }
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
    info!(
        "From {:?} perspective, win: {win:?}, draw: {draw:?}, lost: {lose:?}, unkown: {unkown:?}",
        mat_win.winner
    );
    info!(
        "Index density = {:?}%",
        (file_handler.outcomes.len() * 2 - undefined_outcome) * 100
            / (file_handler.outcomes.len() * 2)
    );
    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Win(i)) {
            info!("Win({}), {:?}", i, nb_win);
        }
    }

    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Lose(i)) {
            info!("Lose({}), {:?}", i, nb_win);
        }
    }
}
