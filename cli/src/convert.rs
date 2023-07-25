use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};

use std::path::PathBuf;

use clap::Args;

use crate::explore::MatOrAll;

/// Convert back and forth helpmate files using the naive indexer and syzygy indexer
#[derive(Args, Debug)]
pub struct Convert {
    #[arg(help = "example \"KQvK\", use special value 'all' to search across all positions", value_parser = MatOrAll::from_str_sequential)]
    mat_or_all: MatOrAll,
    #[arg(long, default_value = "table/")]
    tb_dir: PathBuf,
    #[arg(long, default_value = "syzygy_table/")]
    output_dir: PathBuf,
}

impl Convert {
    pub fn run(&self) {
        for _mat_win in self.mat_or_all.mat_winners(&self.tb_dir, None) {}
    }
}
