mod explore;
mod generate;

pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};

use env_logger::{Builder, Target};
use log::LevelFilter;

use clap::{ArgAction, Parser, Subcommand};

use crate::explore::Explore;
use crate::generate::Generate;

#[cfg(feature = "dhat")]
#[global_allocator]
static DHAT_ALLOCATOR: dhat::Alloc = dhat::Alloc;
// 3 pieces before using index At t-gmax: 19,080,095 bytes (100%) in 47 blocks (100%), avg size 405,959.47 bytes
// 4 pieces before using index At t-gmax: 610,457,858 bytes (100%) in 199 blocks (100%), avg size 3,067,627.43 bytes

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    #[arg(short, long, action = ArgAction::Count, default_value_t = 2)]
    verbose: u8,
    #[arg(
        long,
        help = "If set, logs will not include a timestamp",
        action = ArgAction::SetTrue
    )]
    no_time: bool,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Generate(Generate),
    Explore(Explore),
}

impl Cmd {
    fn run(self) {
        match self {
            Self::Generate(gen) => gen.run(),
            Self::Explore(expl) => expl.run(),
        }
    }
}

fn main() {
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::new_heap();
    let args = Cli::parse();
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

    if args.no_time {
        builder.format_timestamp(None);
    }
    builder.init();
    args.cmd.run()
}
