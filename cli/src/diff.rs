pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, FileHandler, IndexWithTurn};
use log::{debug, error, info, warn};
use std::{path::PathBuf, str::FromStr};

use retroboard::shakmaty::Color;

use clap::Args;

#[derive(Args, Debug)]
pub struct Diff {
    #[arg(help = "example \"KQvK\"", value_parser = Material::from_str)]
    material: Material,
    #[arg(long, default_value = "old_table/")]
    old_tb_dir: PathBuf,
    #[arg(long, default_value = if cfg!(feature = "syzygy") {"syzygy_table/"} else {"table/"})]
    tb_dir: PathBuf,
    #[arg(
        short,
        long,
        help = "Color of the expected winner. If no color is provided, will search for both"
    )]
    winner: Option<Color>,
    #[arg(
        short,
        long,
        default_value_t = usize::MAX,
        help = "Max number of differences to look for"
    )]
    number: usize,
}

impl Diff {
    pub fn run(&self) {
        for winner in self.winner.map(|w| vec![w]).unwrap_or(Color::ALL.into()) {
            info!("Diff-ing {:?} with winner: {winner}", self.material);
            let mat_win = MaterialWinner::new(&self.material, winner);
            let old_file_handler: FileHandler = FileHandler::new(&mat_win, &self.old_tb_dir);
            let file_handler: FileHandler = FileHandler::new(&mat_win, &self.tb_dir);
            self.diff(old_file_handler, file_handler);
        }
    }

    fn diff(&self, old_file_handler: FileHandler, file_handler: FileHandler) {
        let mut old_better = 0;
        let mut new_better = 0;
        if old_file_handler.outcomes.len() != file_handler.outcomes.len() {
            error!(
                "The two tables do not have the same length, old: {}, new {}",
                old_file_handler.outcomes.len(),
                file_handler.outcomes.len()
            )
        }
        for (idx, (old_outcome_bc, outcome_bc)) in old_file_handler
            .outcomes
            .iter()
            .zip(file_handler.outcomes)
            .enumerate()
        {
            for turn in Color::ALL {
                // could be faster to look at the OutcomeU8
                let old_outcome = old_outcome_bc.get_outcome_by_color(turn);
                let outcome = outcome_bc.get_outcome_by_color(turn);
                if old_outcome != outcome {
                    old_better += usize::from(old_outcome > outcome);
                    new_better += usize::from(old_outcome < outcome);

                    #[cfg(not(feature = "syzygy"))]
                    let pos = file_handler.indexer.restore(
                        &self.material,
                        IndexWithTurn {
                            idx: idx as u64,
                            turn,
                        },
                    );
                    #[cfg(feature = "syzygy")]
                    let pos = unreachable!("Syzygy indexer is not reversible");
                    debug!("idx: {idx}, Outcome differs: old {old_outcome:?}, new {outcome:?}");
                    debug!("pos: {pos:?}");
                }
            }
            if self.number <= old_better + new_better {
                break;
            }
        }
        warn!(
            "Found {} differences\nOld is better: {old_better} cases New is better: {new_better}",
            old_better + new_better,
        );
    }
}
