use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, FileHandler, IndexWithTurn, Indexer, Report, Table};
use log::debug;
use retroboard::shakmaty::Color;

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
                for turn in Color::ALL {
                    let outcome = outcome_bc.get_by_color(turn);
                    assert_ne!(outcome, Outcome::Unknown);
                    if outcome != Outcome::Undefined {
                        let rboard = file_handler.indexer.restore(
                            &mat_win.material,
                            IndexWithTurn {
                                idx: idx as u64,
                                turn,
                            },
                        );
                        let syzygy_idx = syzygy_common.indexer().encode(&rboard).usize();
                        // mark as processed because it's already the final outcome
                        syzygy_common.all_pos[syzygy_idx]
                            .set_to(&rboard, Report::Processed(outcome));
                    }
                }
            }
            syzygy_common.all_pos.shrink_to_fit();
            debug!(
                "syzygy all_pos len {} capacity: {} after shrinking",
                syzygy_common.all_pos.len(),
                syzygy_common.all_pos.capacity()
            );
            debug!(
                "naive all_pos len {} capacity: {}",
                file_handler.outcomes.len(),
                file_handler.outcomes.capacity()
            );
            debug!(
                "syzygy is len is {}% smaller, and capacity is {}%",
                (file_handler.outcomes.len() - syzygy_common.all_pos.len())
                    / syzygy_common.all_pos.len(),
                (file_handler.outcomes.capacity() - syzygy_common.all_pos.capacity())
                    / syzygy_common.all_pos.capacity()
            );

            let mut encoder = EncoderDecoder::new(File::create(syzygy_path).unwrap());
            encoder
                .compress(&syzygy_common.all_pos)
                .expect("Compression failed for mat {mat:?}");
        }
    }
}
