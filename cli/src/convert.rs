use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, FileHandler, IndexWithTurn, Report, Table};

use std::{fs::File, path::PathBuf};

use clap::Args;

use crate::explore::MatOrAll;

/// Convert helpmate files using the naive indexer to use syzygy indexer
// check how much space we save
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
        for mat_win in self.mat_or_all.mat_winners(&self.tb_dir, None) {
            let file_handler: FileHandler = FileHandler::new(&mat_win, &self.tb_dir);
            let syzygy_path = self.output_dir.join(format!("{mat_win:?}"));
            let mut syzygy_common: Common<Table> = Common::new(mat_win.clone());
            for (idx, outcome_bc) in file_handler.outcomes.iter().enumerate() {
                for (turn, outcome) in outcome_bc.zip_color() {
                    let rboard = file_handler.indexer.restore(
                        &mat_win.material,
                        IndexWithTurn {
                            idx: idx as u64,
                            turn,
                        },
                    );
                    // mark as processed because it's already the final outcome
                    syzygy_common.all_pos[idx].set_to(&rboard, Report::Processed(outcome.into()));
                }
            }
            let mut encoder = EncoderDecoder::new(File::create(syzygy_path).unwrap());
            encoder
                .compress(&syzygy_common.all_pos)
                .expect("Compression failed for mat {mat:?}");
        }
    }
}
