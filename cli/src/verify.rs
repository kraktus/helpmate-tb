use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{
    DeIndexer, DefaultIndexer, FileHandler, IndexWithTurn, Indexer, OutcomeU8, RetrieveOutcome,
    TablebaseProber,
};
use log::{debug, error, info};
use rustc_hash::FxHashMap;
use std::{path::PathBuf, str::FromStr};

use retroboard::{
    shakmaty::{ByColor, Chess, Color, Position},
    RetroBoard,
};

use clap::Args;

use crate::explore::MatOrAll;

#[derive(Args, Debug)]
pub struct Verify {
    #[arg(help = "example \"KQvK\", use special value 'all' to search across all positions", value_parser = MatOrAll::from_str_sequential)]
    mat_or_all: MatOrAll,
    #[arg(long, default_value = if cfg!(feature = "syzygy") {"syzygy_table/"} else {"table/"})]
    tb_dir: PathBuf,
}

impl Verify {
    pub fn run(&self) {
        for mat_win in self.mat_or_all.mat_winners(&self.tb_dir, None) {
            self.verify_one_mat(mat_win)
        }
    }

    fn verify_one_mat(&self, mat_win: MaterialWinner) {
        info!(
            "Verifying {:?} with winner: {}",
            mat_win.material, mat_win.winner
        );
        let file_handler: FileHandler = FileHandler::new(&mat_win, &self.tb_dir);
        let tb_prober: TablebaseProber = TablebaseProber::new(&mat_win.material, &self.tb_dir);
        for (idx, by_color_outcome) in file_handler.outcomes.iter().enumerate() {
            for turn in Color::ALL {
                let outcome = by_color_outcome.get_by_color(turn);
                assert_ne!(outcome, Outcome::Unknown);
                if outcome != Outcome::Undefined {
                    let idx_with_turn = IndexWithTurn {
                        idx: idx as u64,
                        turn,
                    };
                    let rboard = file_handler
                        .indexer
                        .restore(&mat_win.material, idx_with_turn);
                    let chess: Chess = rboard.clone().into();
                    for m in chess.legal_moves() {
                        let mut chess_after_move = chess.clone();
                        chess_after_move.play_unchecked(&m);
                        let outcome_after_m = tb_prober
                            .retrieve_outcome(&chess_after_move, mat_win.winner)
                            .unwrap();
                        assert_ne!(outcome_after_m, Outcome::Undefined);

                        if outcome_after_m + 1 > outcome {
                            error!("idx: {idx_with_turn:?}, pos: {rboard:?} outcome is {outcome:?}, but after {m:?}, outcome is {outcome_after_m:?}")
                        }
                    }
                }
            }
        }
    }
}
