use crate::{
    index, index_unchecked, indexer::A1_D1_D4, restore_from_index, Common, Descendants, Material,
    Outcome, OutcomeU8, Report, ReportU8, A1_H8_DIAG, UNDEFINED_OUTCOME_BYCOLOR,
};
use log::debug;
use retroboard::shakmaty::{
    Bitboard, Board, ByColor, CastlingMode,
    CastlingMode::Standard,
    Chess,
    Color::{self, White},
    FromSetup, Outcome as ChessOutcome, Piece, Position, PositionError, Setup, Square,
};
use retroboard::RetroBoard;
use std::collections::VecDeque;

use indicatif::ProgressBar;

pub trait WithBoard {
    fn board(&self) -> &Board;
}

impl WithBoard for Board {
    fn board(&self) -> &Board {
        self
    }
}

impl WithBoard for Chess {
    fn board(&self) -> &Board {
        Position::board(self)
    }
}

impl WithBoard for RetroBoard {
    fn board(&self) -> &Board {
        self.board()
    }
}

// Allow to use both `Chess` and `RetroBoard`
pub trait SideToMove: WithBoard {
    // side to **move**, so opposite of side to unmove
    fn side_to_move(&self) -> Color;
}

impl SideToMove for Chess {
    fn side_to_move(&self) -> Color {
        self.turn()
    }
}

impl SideToMove for RetroBoard {
    fn side_to_move(&self) -> Color {
        !self.retro_turn()
    }
}

pub trait SideToMoveGetter {
    type T;
    // chose `get_by_color` and not `get` not to shadow the original methods
    fn get_by_color(&self, color: Color) -> Self::T;
    fn get_by_pos(&self, pos: &impl SideToMove) -> Self::T {
        self.get_by_color(pos.side_to_move())
    }
    fn set_to(&mut self, pos: &impl SideToMove, t: Self::T);
}

impl SideToMoveGetter for ByColor<ReportU8> {
    type T = Report;
    fn get_by_color(&self, color: Color) -> Self::T {
        self.get(color).into()
    }
    fn set_to(&mut self, pos: &impl SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t.into();
    }
}

impl SideToMoveGetter for ByColor<OutcomeU8> {
    type T = Outcome;
    fn get_by_color(&self, color: Color) -> Self::T {
        self.get(color).into()
    }
    fn set_to(&mut self, pos: &impl SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t.into();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexWithTurn {
    pub idx: u64,
    pub turn: Color,
}

// the index is independant of the turn, so must be stored separately
#[derive(Debug, Clone, Default)]
pub struct Queue {
    // depending on the material configuration can be either won or drawn position
    pub desired_outcome_pos_to_process: VecDeque<IndexWithTurn>,
    pub losing_pos_to_process: VecDeque<IndexWithTurn>,
}

pub const A1_H1_H8: Bitboard = Bitboard(0x80c0_e0f0_f8fc_feff);
// const A8_A2_H7: Bitboard = A1_H1_H8.flip_diagonal().without_const(A1_H8_DIAG);

// type PosHandler = fn(&mut Common, &mut Queue, &Descendants, &Chess, u64, usize);

pub trait PosHandler {
    fn handle_position(
        &mut self,
        common: &mut Common,
        queue: &mut Queue,
        tablebase: &Descendants,
        chess: &Chess,
        idx: IndexWithTurn,
        all_pos_idx: usize,
    );
}

/// handler used when generating the helpmate tablebase
/// another handler can be found in `syzygy_check.rs`
struct DefaultGeneratorHandler;

impl PosHandler for DefaultGeneratorHandler {
    fn handle_position(
        &mut self,
        common: &mut Common,
        queue: &mut Queue,
        tablebase: &Descendants,
        chess: &Chess,
        idx: IndexWithTurn,
        all_pos_idx: usize,
    ) {
        match chess.outcome() {
            Some(ChessOutcome::Decisive { winner }) => {
                // we know the result is exact, since the game is over
                let outcome = Report::Processed(if winner == common.winner {
                    assert!(common.can_mate());
                    Outcome::Win(0)
                } else {
                    Outcome::Lose(0)
                });
                common.all_pos[all_pos_idx].set_to(chess, outcome);
                if winner == common.winner {
                    queue.desired_outcome_pos_to_process.push_back(idx);
                } else {
                    queue.losing_pos_to_process.push_back(idx);
                }
            }

            Some(ChessOutcome::Draw) => {
                common.all_pos[all_pos_idx].set_to(chess, Report::Processed(Outcome::Draw));
                if !common.can_mate() {
                    queue.desired_outcome_pos_to_process.push_back(idx);
                }
            }
            None => {
                common.all_pos[all_pos_idx].set_to(
                    chess,
                    Report::Unprocessed(
                        tablebase
                            .outcome_from_captures_promotion(chess, common.winner)
                            .unwrap_or(Outcome::Unknown),
                    ),
                );
            }
        }
    }
}

/// Struct that only handle the generation phase of the tablebase building process
/// See `Tagger` for the backward algorithm part.
pub struct Generator<T> {
    common: Common,
    tablebase: Descendants, // access to the DTM of descendants (different material config, following a capture/promotion)
    pb: ProgressBar,
    queue: Queue,
    pos_handler: T,
}

impl Generator<DefaultGeneratorHandler> {
    #[must_use]
    pub fn new(common: Common) -> Self {
        Self::new_with_pos_handler(DefaultGeneratorHandler, common)
    }
}

impl<T: PosHandler> Generator<T> {
    pub fn new_with_pos_handler(pos_handler: T, common: Common) -> Self {
        let pb = common.get_progress_bar();
        Self {
            pb,
            tablebase: Descendants::new(&common.material),
            common,
            queue: Queue::default(),
            pos_handler,
        }
    }

    pub fn get_result(self) -> (Queue, Common, T) {
        (self.queue, self.common, self.pos_handler)
    }

    fn generate_positions_internal(
        &mut self,
        piece_vec: &[Piece],
        setup: &Setup,
        last_piece_and_square: (Piece, Square),
    ) {
        match piece_vec {
            [piece, tail @ ..] => {
                let squares = self.valid_squares(
                    &setup.board,
                    *piece,
                    last_piece_and_square.0,
                    last_piece_and_square.1,
                );
                for sq in squares {
                    if setup.board.piece_at(sq).is_none() {
                        let mut new_setup = setup.clone();
                        new_setup.board.set_piece_at(sq, *piece);
                        self.generate_positions_internal(tail, &new_setup, (*piece, sq));
                    }
                }
            }
            [] => self.check_setup(setup),
        }
    }

    #[inline]
    fn valid_squares(
        &self,
        board: &Board,
        piece: Piece,
        last_piece: Piece,
        last_square: Square,
    ) -> Bitboard {
        if last_piece == piece {
            // by convention the former piece put on the board
            // should have a "higher" square than the later to avoid
            // generating the same position but with identical pieces swapped
            (0..last_square.into())
                .map(unsafe { |sq| Square::new_unchecked(sq) })
                .collect()
        }
        // Do not restrict duplicate pieces as they already have other constraints
        // and combining with this one resulting in the generating function not to be surjective anymore
        else if (self.common.material.by_piece(piece) == 1)
            && A1_H8_DIAG.is_superset(board.occupied())
        {
            A1_H1_H8
        } else {
            Bitboard::FULL
        }
    }

    fn check_setup(&mut self, setup: &Setup) {
        // setup is complete, check if valid
        for color in Color::ALL {
            let mut valid_setup = setup.clone();
            valid_setup.turn = color;
            self.common.counter += 1;
            if self.common.counter % 100_000 == 0 {
                self.pb.set_position(self.common.counter);
            }
            if let Ok(chess) = to_chess_with_illegal_checks(valid_setup.clone()) {
                let rboard = RetroBoard::from_setup(valid_setup, Standard)
                    .expect("if chess is valid then rboard should be too");
                let idx = index_unchecked(&rboard); // by construction positions generated have white king in the a1-d1-d4 corner
                let all_pos_idx = self.common.index_table().encode(&chess);
                // if format!("{}", rboard.board().board_fen(Bitboard::EMPTY))
                //     == "7k/2R5/8/8/3K4/8/8/1R6"
                // {
                //     println!("TEST {rboard:?}")
                // };
                // Check that position is generated for the first time/index schema is injective
                // We consider the syzygy indexer trusty enough for pawnless positions to allow for
                // duplicates
                if Outcome::Undefined
                    == self.common.all_pos[all_pos_idx]
                        .get_by_pos(&chess)
                        .outcome()
                {
                    // only handle the position if it's not a duplicate
                    self.pos_handler.handle_position(
                        &mut self.common,
                        &mut self.queue,
                        &self.tablebase,
                        &chess,
                        idx,
                        all_pos_idx,
                    );
                } else {
                    assert!(
                        !self.common.material.has_pawns(),
                        "Index {all_pos_idx} already generated, board: {rboard:?}"
                    );
                }
            }
        }
    }

    pub fn generate_positions(&mut self) {
        let piece_vec = self.common.material.pieces_without_white_king();
        self.common.counter = 0;
        for white_king_sq in A1_D1_D4 {
            let mut new_setup = Setup::empty();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(&piece_vec, &new_setup, (White.king(), white_king_sq))
        }
        self.pb.finish_and_clear();
        debug!("all_pos_vec capacity: {}", self.common.all_pos.capacity());
        while Some(&UNDEFINED_OUTCOME_BYCOLOR) == self.common.all_pos.last() {
            self.common.all_pos.pop();
        }

        self.common.all_pos.shrink_to_fit();
        debug!(
            "all_pos_vec capacity: {} after shrinking",
            self.common.all_pos.capacity()
        );
    }
}

/// When all legal positions have already been generated, start backward algo from all mates positions
/// and tag them (ie associates an Outcome)
#[derive(Debug)]
struct Tagger {
    common: Common,
    pb: ProgressBar,
}

impl Tagger {
    pub fn new(common: Common) -> Self {
        let pb = common.get_progress_bar();
        Self { common, pb }
    }

    pub fn process_positions(&mut self, queue: &mut VecDeque<IndexWithTurn>) {
        self.common.counter = 0;
        while let Some(idx) = queue.pop_front() {
            self.common.counter += 1;
            if self.common.counter % 100_000 == 0 {
                self.pb.set_position(self.common.counter);
            }
            let rboard = restore_from_index(&self.common.material, idx);
            let out: Outcome = self
                .common
                .all_pos
                .get(self.common.index_table().encode(&rboard))
                .map_or_else(
                    || {
                        panic!(
                            "idx get_by_pos {}, idx recomputed {}, rboard {:?}",
                            idx.idx,
                            index(&rboard).idx,
                            rboard
                        )
                    },
                    |bc| bc.get_by_pos(&rboard),
                )
                .outcome();
            assert_ne!(out, Outcome::Undefined);
            for m in rboard.legal_unmoves() {
                let mut rboard_after_unmove = rboard.clone();
                rboard_after_unmove.push(&m);
                // let chess_after_unmove: Chess = rboard_after_unmove.clone().into();
                let idx_after_unmove = index(&rboard_after_unmove);
                let idx_all_pos_after_unmove =
                    self.common.index_table().encode(&rboard_after_unmove);
                match self.common.all_pos[idx_all_pos_after_unmove].get_by_pos(&rboard_after_unmove)
                {
                    Report::Processed(Outcome::Undefined) => {
                        panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                    }
                    Report::Unprocessed(fetched_outcome) => {
                        // we know the position is unprocessed
                        queue.push_back(idx_after_unmove);
                        let processed_outcome = Report::Processed((out + 1).max(fetched_outcome));
                        self.common.all_pos[idx_all_pos_after_unmove]
                            .set_to(&rboard_after_unmove, processed_outcome);
                    }
                    Report::Processed(_) => (),
                }
            }
        }

        // all positions that are unknown at the end are drawn
        for report_bc in &mut self.common.all_pos {
            for report in report_bc.iter_mut() {
                if Report::Unprocessed(Outcome::Unknown) == Report::from(*report) {
                    *report = ReportU8::from(Report::Processed(Outcome::Draw))
                }
            }
        }
        self.pb.finish_and_clear();
    }
}

impl From<Tagger> for Common {
    fn from(t: Tagger) -> Self {
        t.common
    }
}

pub struct TableBaseBuilder;

impl TableBaseBuilder {
    #[must_use]
    pub fn build(material: Material, winner: Color) -> Common {
        let common = Common::new(material, winner);
        let mut generator = Generator::new(common);
        generator.generate_positions();
        let (mut queue, common, _) = generator.get_result();
        debug!("nb pos {:?}", common.all_pos.len());
        debug!("counter {:?}", common.counter);
        debug!(
            "nb {:?} {} {:?}",
            common.winner,
            if common.can_mate() {
                "mate"
            } else {
                "stalemate/capture resulting in draw"
            },
            queue.desired_outcome_pos_to_process.len()
        );
        debug!(
            "nb {:?} mates {:?}",
            !common.winner,
            queue.losing_pos_to_process.len()
        );
        let mut tagger = Tagger::new(common);
        // need to process FIRST winning positions, then losing ones.
        tagger.process_positions(&mut queue.desired_outcome_pos_to_process);
        tagger.process_positions(&mut queue.losing_pos_to_process);
        tagger.into()
    }
}

pub fn to_chess_with_illegal_checks(setup: Setup) -> Result<Chess, PositionError<Chess>> {
    Chess::from_setup(setup, CastlingMode::Standard)
        .or_else(retroboard::shakmaty::PositionError::ignore_impossible_check)
}
#[cfg(test)]
mod tests {
    use super::*;
    use retroboard::shakmaty::fen::Fen;

    #[test]
    fn test_a1_h8_bb() {
        assert_eq!(A1_H1_H8, Bitboard(9_277_662_557_957_324_543))
    }

    #[test]
    fn test_side_to_move() {
        let fen = "4k3/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQ - 0 1";
        let rboard = RetroBoard::new_no_pockets(fen).unwrap();
        let chess: Chess = Fen::from_ascii(fen.as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        assert_eq!(rboard.side_to_move(), White);
        assert_eq!(chess.side_to_move(), White);
    }
}
