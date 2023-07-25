use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, Descendants, FileHandler, IndexWithTurn, Indexer, RetrieveOutcome};
use log::{debug, error, info};

use std::path::PathBuf;

use retroboard::{
    shakmaty::{Chess, Color, Position},
    RetroBoard,
};

use clap::Args;

use crate::explore::MatOrAll;

/// Perform sanity checks on given helpmate files
#[derive(Args, Debug)]
pub struct Verify {
    #[arg(help = "example \"KQvK\", use special value 'all' to search across all positions", value_parser = MatOrAll::from_str_sequential)]
    mat_or_all: MatOrAll,
    #[arg(long, default_value = "table/")]
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
        let descendants: Descendants = Descendants::new(&mat_win, &self.tb_dir);
        debug!("outcomes len: {}", file_handler.outcomes.len());
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
                        let outcome_after_m = if m.is_capture() {
                            descendants
                                .retrieve_outcome(&chess_after_move, mat_win.winner)
                                .unwrap()
                        } else {
                            let idx_after_m = file_handler.indexer.encode(&chess_after_move);
                            file_handler.outcomes[idx_after_m.usize()].get_by_pos(&chess_after_move)
                        };
                        assert_ne!(outcome_after_m, Outcome::Undefined);

                        if outcome_after_m + 1 > outcome {
                            error!("idx: {idx_with_turn:?}, pos: {rboard:?} outcome is {outcome:?}, but after {m:?}, outcome is {outcome_after_m:?}");
                            debug!(
                                "unmoves after the move: {:?}",
                                RetroBoard::from(chess_after_move).legal_unmoves()
                            );
                        }
                    }
                }
            }
            if idx % 100_000 == 0 {
                debug!("idx: {idx}")
            }
        }
    }
}
