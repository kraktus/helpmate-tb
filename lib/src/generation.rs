use crate::{
    indexer::{DeIndexer, Indexer, A1_D1_D4},
    queue::{OneQueue, Queue},
    Common, DefaultReversibleIndexer, Descendants, Material, Outcome, OutcomeU8, Report, ReportU8,
    A1_H8_DIAG, UNDEFINED_OUTCOME_BYCOLOR,
};
use log::{debug, error, info, trace, warn};
use retroboard::shakmaty::{
    Bitboard, Board, ByColor, CastlingMode,
    CastlingMode::Standard,
    Chess,
    Color::{self, White},
    FromSetup, Outcome as ChessOutcome, Piece, Position, PositionError, Setup, Square,
};
use retroboard::RetroBoard;
use std::path::Path;

use indicatif::ProgressBar;

pub trait WithBoard {
    fn board(&self) -> &Board;
}

impl WithBoard for Board {
    fn board(&self) -> &Board {
        self
    }
}

impl<'a> WithBoard for &'a Board {
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

impl WithBoard for (Board, Color) {
    fn board(&self) -> &Board {
        &self.0
    }
}

impl<'a> WithBoard for (&'a Board, Color) {
    fn board(&self) -> &Board {
        &self.0
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

impl SideToMove for (Board, Color) {
    fn side_to_move(&self) -> Color {
        self.1
    }
}

impl<'a> SideToMove for (&'a Board, Color) {
    fn side_to_move(&self) -> Color {
        self.1
    }
}

pub trait SideToMoveGetter {
    type T;
    // chose `get_by_color` and not `get` not to shadow the original methods
    fn get_by_color(&self, color: Color) -> Self::T;
    fn get_outcome_by_color(&self, color: Color) -> Outcome;
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

    fn get_outcome_by_color(&self, color: Color) -> Outcome {
        self.get_by_color(color).outcome()
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

    fn get_outcome_by_color(&self, color: Color) -> Outcome {
        self.get_by_color(color)
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

impl IndexWithTurn {
    #[must_use]
    pub fn usize(&self) -> usize {
        self.idx
            .try_into()
            .expect("Only 64bits and larger are supported")
    }
}

pub const A1_H1_H8: Bitboard = Bitboard(0x80c0_e0f0_f8fc_feff);
// const A8_A2_H7: Bitboard = A1_H1_H8.flip_diagonal().without_const(A1_H8_DIAG);

// type PosHandler = fn(&mut Common, &mut Queue, &Descendants, &Chess, u64, usize);

pub trait PosHandler<I> {
    fn handle_position(
        &mut self,
        common: &mut Common<I>,
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

impl<I> PosHandler<I> for DefaultGeneratorHandler {
    fn handle_position(
        &mut self,
        common: &mut Common<I>,
        queue: &mut Queue,
        tablebase: &Descendants,
        chess: &Chess,
        idx: IndexWithTurn,
        all_pos_idx: usize,
    ) {
        match chess.outcome() {
            Some(ChessOutcome::Decisive { winner }) => {
                // we know the result is exact, since the game is over
                let outcome = Report::Unprocessed(if winner == common.winner {
                    assert!(common.can_mate());
                    Outcome::Win(0)
                } else {
                    Outcome::Lose(0)
                });
                common.all_pos[all_pos_idx].set_to(chess, outcome);
                if winner == common.winner {
                    queue.desired_outcome_pos_to_process.push(idx);
                } else {
                    queue.losing_pos_to_process.push(idx);
                }
            }

            Some(ChessOutcome::Draw) => {
                // TODO verify the thing with processed/unprocessed
                let can_mate = common.can_mate();
                common.all_pos[all_pos_idx].set_to(
                    chess,
                    if !can_mate {
                        // need to be unprocessed because when we cannot mate
                        // we work backward from stalemate/captures that lead to insufficient material
                        Report::Unprocessed(Outcome::Draw)
                    } else {
                        Report::Processed(Outcome::Draw)
                    },
                );
                if !common.can_mate() {
                    queue.desired_outcome_pos_to_process.push(idx);
                }
            }
            None => {
                let (fetched_outcome, _) = tablebase
                    .outcome_from_captures_promotion(chess, common.winner)
                    .unwrap_or((Outcome::Unknown, false));
                // fetched outcome should always be unprocessed
                // in case of **drawn** unreachable positions (and only when we try to win),
                // those will be left unprocessed
                common.all_pos[all_pos_idx].set_to(chess, Report::Unprocessed(fetched_outcome));
            }
        }
    }
}

/// Struct that only handle the generation phase of the tablebase building process
/// See `Tagger` for the backward algorithm part.
pub struct Generator<T, I> {
    common: Common<I>,
    tablebase: Descendants, // access to the DTM of descendants (different material config, following a capture/promotion)
    pb: ProgressBar,
    queue: Queue,
    pos_handler: T,
}

impl<I: Indexer> Generator<DefaultGeneratorHandler, I> {
    #[must_use]
    pub fn new(common: Common<I>, tablebase_path: &Path) -> Self {
        Self::new_with_pos_handler(DefaultGeneratorHandler, common, tablebase_path)
    }
}

impl<T: PosHandler<I>, I: Indexer> Generator<T, I> {
    pub fn new_with_pos_handler(pos_handler: T, common: Common<I>, tablebase_dir: &Path) -> Self {
        let pb = common.get_progress_bar().with_message("Gen pos");
        Self {
            pb,
            tablebase: Descendants::new(&common.material, tablebase_dir),
            common,
            queue: Queue::default(),
            pos_handler,
        }
    }

    pub fn get_result(self) -> (Queue, Common<I>, T) {
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
                let idx = self.queue.encode(&rboard); // The position by construction is unfortunately not always canonical, so best to re-check when encoding
                let all_pos_idx = self.common.indexer().encode(&chess).usize();
                // if format!("{}", rboard.board().board_fen(Bitboard::EMPTY))
                //     == "7k/2R5/8/8/3K4/8/8/1R6"
                // {
                //     println!("TEST {rboard:?}")
                // };
                // if all_pos_idx == 942601 {
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
                        // In positions without pawns with duplicate pieces, duplicate indexes are tolerated
                        // because could not find a way to generate positions without those
                        !self.common.material.has_pawns()
                            && self.common.material.min_like_man() > 1,
                        "Index {all_pos_idx} already generated, board: {rboard:?}"
                    );
                }
            }
        }
    }

    pub fn generate_positions(&mut self) {
        let piece_vec = self.common.material.pieces_without_white_king();
        self.common.counter = 0;
        let all_pos_vec_capacity_before_gen = self.common.all_pos.capacity();
        debug!("all_pos_vec capacity before generating: {all_pos_vec_capacity_before_gen}");
        for white_king_sq in A1_D1_D4 {
            let mut new_setup = Setup::empty();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(&piece_vec, &new_setup, (White.king(), white_king_sq))
        }
        self.pb.finish_and_clear();
        let all_pos_vec_capacity_after_gen = self.common.all_pos.capacity();
        debug!("all_pos_vec capacity after generating: {all_pos_vec_capacity_after_gen}");
        // can this actually happen in practice or will the common use of Index make it panic during the process?
        if all_pos_vec_capacity_after_gen > all_pos_vec_capacity_before_gen {
            warn!("For material {:?}, all_pos capacity was not enough to generate the positions, before {all_pos_vec_capacity_before_gen}, after {all_pos_vec_capacity_after_gen}", self.common.material)
        }
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
struct Tagger<T = DefaultReversibleIndexer> {
    common: Common,
    pb: ProgressBar,
    reversible_indexer: T,
}

impl<T: From<Material>> Tagger<T> {
    pub fn new(common: Common) -> Self {
        let pb = common.get_progress_bar().with_message("Tagging pos");
        Self {
            reversible_indexer: T::from(common.material.clone()),
            common,
            pb,
        }
    }
}

impl<T: Indexer + DeIndexer> Tagger<T> {
    pub fn process_positions(&mut self, _: Queue) {
        // need to process FIRST winning positions, then losing ones.
        self.process_one_queue(true);
        self.process_one_queue(false);

        for report_bc in &mut self.common.all_pos.iter_mut() {
            for turn in Color::ALL {
                let report = report_bc.get_mut(turn);
                match Report::from(*report) {
                    Report::Unprocessed(Outcome::Unknown) => {
                        *report = ReportU8::from(Report::Processed(Outcome::Draw))
                    }
                    Report::Unprocessed(not_unknown) => {
                        // some positions are unreachable, like "8/8/8/8/8/2N5/1B6/k2K4 b - - 0 1"
                        // so their outcome is only determined by the outcome of the capture
                        debug_assert!(not_unknown == Outcome::Draw)
                    }
                    Report::Processed(_) => {}
                }
            }
        }
    }

    // if `desired_outcome_to_process` is set to `true`, we go from `Win(0)` to Win(1) ...
    // if `false`, go from `Lose(0)` to Lose(1) Lose(2) ...
    pub fn process_one_queue(&mut self, desired_outcome_to_process: bool) {
        self.common.counter = 0;
        let mut at_least_one_pos_processed = true;
        let mut desired_outcome = if desired_outcome_to_process {
            if self.common.can_mate() {
                Outcome::Win(0)
            } else {
                Outcome::Draw
            }
        } else {
            Outcome::Lose(0)
        };
        while at_least_one_pos_processed {
            at_least_one_pos_processed = false;
            let desired_report_u8: ReportU8 = Report::Unprocessed(desired_outcome).into();
            let mut one_queue = OneQueue::new_empty(self.common.all_pos.len());
            for (idx, report_u8_bc) in self.common.all_pos.iter().enumerate() {
                for turn in Color::ALL {
                    if &desired_report_u8 == report_u8_bc.get(turn) {
                        let idx_with_turn = IndexWithTurn {
                            idx: idx as u64,
                            turn,
                        };
                        one_queue.mate_in_n.push_back(idx_with_turn);
                        at_least_one_pos_processed = true;
                        self.common.counter += 1;
                        if self.common.counter % 100_000 == 0 {
                            self.pb.set_position(self.common.counter);
                        }
                        let rboard = self
                            .reversible_indexer
                            .restore(&self.common.material, idx_with_turn);
                        for m in rboard.legal_unmoves() {
                            let mut rboard_after_unmove = rboard.clone();
                            rboard_after_unmove.push(&m);
                            let idx_all_pos_after_unmove =
                                self.common.indexer().encode(&rboard_after_unmove);
                            match self.common.all_pos[idx_all_pos_after_unmove.usize()]
                                .get_by_pos(&rboard_after_unmove)
                            {
                                Report::Processed(Outcome::Undefined) => {
                                    panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                                }
                                Report::Unprocessed(fetched_outcome) => {
                                    // we know the position is unprocessed
                                    assert!(fetched_outcome <= desired_outcome);
                                    one_queue
                                        .mate_in_n_plus_1
                                        .push_back(idx_all_pos_after_unmove)
                                }
                                Report::Processed(_) => (),
                            }
                        }
                    }
                }
            }

            while let Some(idx_with_turn_n) = one_queue.mate_in_n.pop_front() {
                *self.common.all_pos[idx_with_turn_n.usize()].get_mut(idx_with_turn_n.turn) =
                    Report::Processed(desired_outcome).into();
            }

            while let Some(idx_with_turn_n_plus_1) = one_queue.mate_in_n_plus_1.pop_front() {
                // a position mate in N can reach a another position mate in N, so we need to double check
                // it has not been processed already before setting its value
                match self.common.all_pos[idx_with_turn_n_plus_1.usize()]
                    .get_by_color(idx_with_turn_n_plus_1.turn)
                {
                    // if the report is `mate_in_n_plus_1` queue it means that
                    // it was unprocessed and has been processed by the loop just above
                    // so it should be equal to the desired outcome
                    Report::Processed(outcome) => assert!(outcome == desired_outcome),
                    Report::Unprocessed(_) => {
                        *self.common.all_pos[idx_with_turn_n_plus_1.usize()]
                            .get_mut(idx_with_turn_n_plus_1.turn) =
                            Report::Unprocessed(desired_outcome + 1).into()
                    }
                }
            }

            desired_outcome = desired_outcome + 1; // one move further from mate
        }
        self.pb.finish_and_clear();
    }
}

impl<T> From<Tagger<T>> for Common {
    fn from(t: Tagger<T>) -> Self {
        t.common
    }
}

pub struct TableBaseBuilder;

impl TableBaseBuilder {
    #[must_use]
    pub fn build(material: Material, winner: Color, tablebase_dir: &Path) -> Common {
        let common = Common::new(material, winner);
        let mut generator = Generator::new(common, tablebase_dir);
        generator.generate_positions();
        let (queue, common, _): (Queue, Common, DefaultGeneratorHandler) = generator.get_result();
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
        // Should be the same indexer than for `Queue`
        let mut tagger: Tagger = Tagger::new(common);
        tagger.process_positions(queue);
        tagger.into()
    }
}

#[allow(clippy::result_large_err)]
pub fn to_chess_with_illegal_checks(setup: Setup) -> Result<Chess, PositionError<Chess>> {
    Chess::from_setup(setup, CastlingMode::Standard).or_else(PositionError::ignore_impossible_check)
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
