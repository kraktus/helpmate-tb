use crate::{
    index, index_unchecked, restore_from_index, Descendants, Material, Outcome, OutcomeU8, Report,
    ReportU8, Reports, Table, A1_H8_DIAG, UNDEFINED_OUTCOME_BYCOLOR,
};
use log::debug;
use retroboard::shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, CastlingMode::Standard, Chess, Color, Color::White,
    FromSetup, Outcome as ChessOutcome, Piece, Position, PositionError, Setup,
};
use retroboard::RetroBoard;
use std::collections::VecDeque;

use indicatif::{ProgressBar, ProgressStyle};

// Allow to use both `Chess` and `RetroBoard`
// TODO replace all `dyn SideToMove` by enum using `enum_trait` crate for example
pub trait SideToMove {
    // side to **move**, so opposite of side to unmove
    fn side_to_move(&self) -> Color;
    fn board(&self) -> &Board;
}

impl SideToMove for Chess {
    fn side_to_move(&self) -> Color {
        self.turn()
    }
    fn board(&self) -> &Board {
        Position::board(self)
    }
}

impl SideToMove for RetroBoard {
    fn side_to_move(&self) -> Color {
        !self.retro_turn()
    }

    fn board(&self) -> &Board {
        self.board()
    }
}

pub trait SideToMoveGetter {
    type T;
    // chose `get_by_color` and not `get` not to shadow the original methods
    fn get_by_color(&self, color: Color) -> Self::T;
    fn get_by_pos(&self, pos: &dyn SideToMove) -> Self::T {
        self.get_by_color(pos.side_to_move())
    }
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T);
}

impl SideToMoveGetter for ByColor<ReportU8> {
    type T = Report;
    fn get_by_color(&self, color: Color) -> Self::T {
        self.get(color).into()
    }
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t.into();
    }
}

impl SideToMoveGetter for ByColor<OutcomeU8> {
    type T = Outcome;
    fn get_by_color(&self, color: Color) -> Self::T {
        self.get(color).into()
    }
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t.into();
    }
}

#[derive(Debug, Clone)]
pub struct Queue {
    // depending on the material configuration can be either won or drawn position
    pub desired_outcome_pos_to_process: VecDeque<u64>,
    pub losing_pos_to_process: VecDeque<u64>,
}

const A1_H1_H8: Bitboard = Bitboard(0x80c0e0f0f8fcfeff);

#[derive(Debug)]
pub struct Common {
    pub all_pos: Reports,
    pub winner: Color,
    pub counter: u64,
    pub material: Material,
    can_mate: bool, // if `true`, the desired outcome is winning, otherwise it's to draw
    index_table: Table,
}

impl Common {
    fn get_progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(get_nb_pos(&self.material));
        pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));
        pb
    }
}

impl Common {
    pub fn new(material: Material, winner: Color) -> Self {
        Self {
            index_table: Table::new(&material),
            all_pos: vec![UNDEFINED_OUTCOME_BYCOLOR; get_nb_pos(&material) as usize / 10 * 9], // heuristic, less than 90% of pos are legals. Takes x2 (because each stored element is in fact 1 position, but with black and white to turn) more than number of legal positions
            winner,
            counter: 0,
            can_mate: material.can_mate(winner),
            material,
        }
    }
}

/// Struct that only handle the generation phase of the tablebase building process
/// See `Tagger` for the backward algorithm part.
#[derive(Debug)]
struct Generator {
    common: Common,
    tablebase: Descendants, // access to the DTM of descendants (different material config, following a capture/promotion)
    pb: ProgressBar,
    queue: Queue,
}

impl Generator {
    pub fn new(common: Common) -> Self {
        let pb = common.get_progress_bar();
        Self {
            pb,
            tablebase: Descendants::new(&common.material),
            common,
            queue: Queue::default(),
        }
    }

    pub fn get_result(self) -> (Queue, Common) {
        (self.queue, self.common)
    }

    fn generate_positions_internal(&mut self, piece_vec: &[Piece], setup: Setup) {
        match piece_vec {
            [piece, tail @ ..] => {
                let squares = if A1_H8_DIAG.is_superset(setup.board.occupied()) {
                    A1_H1_H8
                } else {
                    Bitboard::FULL // white king handled in `generate_positions`
                };
                for sq in squares {
                    if setup.board.piece_at(sq).is_none() {
                        let mut new_setup = setup.clone();
                        new_setup.board.set_piece_at(sq, *piece);
                        self.generate_positions_internal(tail, new_setup);
                    }
                }
            }
            [] => self.check_position(setup),
        }
    }

    fn check_position(&mut self, setup: Setup) {
        // setup is complete, check if valid
        for color in Color::ALL {
            let mut valid_setup = setup.clone();
            valid_setup.turn = color;
            self.common.counter += 1;
            if self.common.counter % 100000 == 0 {
                self.pb.set_position(self.common.counter);
            }
            if let Ok(chess) = to_chess_with_illegal_checks(valid_setup.clone()) {
                let rboard =
                    RetroBoard::from_setup(valid_setup, Standard) // DEBUG
                        .expect("if chess is valid then rboard should be too");
                // let expected_rboard = RetroBoard::new_no_pockets("8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1").unwrap();
                let idx = index_unchecked(&rboard); // by construction positions generated have white king in the a1-d1-d4 corner
                let all_pos_idx = self.common.index_table.encode(&chess);
                // Check that position is generated for the first time/index schema is injective
                if Outcome::Undefined
                    != self.common.all_pos[all_pos_idx]
                        .get_by_pos(&chess)
                        .outcome()
                {
                    panic!("Index {all_pos_idx} already generated, board: {rboard:?}");
                }
                self.handle_outcome_of_legal_position(&chess, idx, all_pos_idx);
            }
        }
    }

    fn handle_outcome_of_legal_position(&mut self, chess: &Chess, idx: u64, all_pos_idx: usize) {
        match chess.outcome() {
            Some(ChessOutcome::Decisive { winner }) => {
                // we know the result is exact, since the game is over
                let outcome = Report::Processed(if winner == self.common.winner {
                    assert!(self.common.can_mate);
                    Outcome::Win(0)
                } else {
                    Outcome::Lose(0)
                });
                self.common.all_pos[all_pos_idx].set_to(chess, outcome);
                if winner == self.common.winner {
                    self.queue.desired_outcome_pos_to_process.push_back(idx);
                } else {
                    self.queue.losing_pos_to_process.push_back(idx);
                }
            }

            Some(ChessOutcome::Draw) => {
                self.common.all_pos[all_pos_idx].set_to(chess, Report::Processed(Outcome::Draw));
                if !self.common.can_mate {
                    self.queue.desired_outcome_pos_to_process.push_back(idx);
                }
            }
            None => {
                self.common.all_pos[all_pos_idx].set_to(
                    chess,
                    Report::Unprocessed(
                        self.tablebase
                            .outcome_from_captures_promotion(&chess, self.common.winner)
                            .unwrap_or(Outcome::Unknown),
                    ),
                );
            }
        }
    }

    pub fn generate_positions(&mut self) {
        let piece_vec = self.common.material.pieces_without_white_king();
        self.common.counter = 0;
        let white_king_bb = Bitboard(135007759); // a1-d1-d4 triangle
        println!("{:?}", white_king_bb.0);
        for white_king_sq in white_king_bb {
            let mut new_setup = Setup::empty();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(&piece_vec, new_setup)
        }
        self.pb.finish_with_message("positions generated");
        println!("all_pos_vec capacity: {}", self.common.all_pos.capacity());
        while Some(&UNDEFINED_OUTCOME_BYCOLOR) == self.common.all_pos.last() {
            self.common.all_pos.pop();
        }

        self.common.all_pos.shrink_to_fit();
        println!(
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
        Self { pb, common }
    }

    pub fn process_positions(&mut self, queue: &mut VecDeque<u64>) {
        self.common.counter = 0;
        loop {
            if let Some(idx) = queue.pop_front() {
                self.common.counter += 1;
                if self.common.counter % 100000 == 0 {
                    self.pb.set_position(self.common.counter);
                }
                let rboard = restore_from_index(&self.common.material, idx);
                let out: Outcome = self
                    .common
                    .all_pos
                    .get(self.common.index_table.encode(&rboard))
                    .map(|bc| bc.get_by_pos(&rboard))
                    .unwrap_or_else(|| {
                        panic!(
                            "idx get_by_pos {}, idx recomputed {}, rboard {:?}",
                            idx,
                            index(&rboard),
                            rboard
                        )
                    })
                    .outcome();
                assert_ne!(out, Outcome::Undefined);
                for m in rboard.legal_unmoves() {
                    let mut rboard_after_unmove = rboard.clone();
                    rboard_after_unmove.push(&m);
                    // let chess_after_unmove: Chess = rboard_after_unmove.clone().into();
                    let idx_after_unmove = index(&rboard_after_unmove);
                    let idx_all_pos_after_unmove =
                        self.common.index_table.encode(&rboard_after_unmove);
                    match self.common.all_pos[idx_all_pos_after_unmove]
                        .get_by_pos(&rboard_after_unmove)
                    {
                        Report::Processed(Outcome::Undefined) => {
                            panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                        }
                        Report::Unprocessed(fetched_outcome) => {
                            // we know the position is unprocessed
                            queue.push_back(idx_after_unmove);
                            let processed_outcome =
                                Report::Processed((out + 1).max(fetched_outcome));
                            // if processed_outcome.outcome() < Outcome::Draw {
                            //     // DEBUG
                            //     println!("Lost position, outcome: {processed_outcome:?}, r");
                            // }
                            self.common.all_pos[idx_all_pos_after_unmove]
                                .set_to(&rboard_after_unmove, processed_outcome);
                        }
                        Report::Processed(_) => (),
                    }
                }
            } else {
                break;
            }
        }
        // TODO once finished check that no legal positions are unprocessed
        self.pb.finish_with_message("positions processed");
    }
}

impl From<Tagger> for Common {
    fn from(t: Tagger) -> Self {
        t.common
    }
}

pub struct TableBaseBuilder;

impl TableBaseBuilder {
    pub fn build(material: Material, winner: Color) -> Common {
        let common = Common::new(material, winner);
        let mut generator = Generator::new(common);
        generator.generate_positions();
        let (mut queue, common) = generator.get_result();
        debug!("nb pos {:?}", common.all_pos.len());
        debug!("counter {:?}", common.counter);
        debug!(
            "nb {:?} mates {:?}",
            common.winner,
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

#[inline]
fn get_nb_pos(mat: &Material) -> u64 {
    // white king is already included in `material.count()`, so substract it, and multiply by 10 instead, real number of cases the white king can go on
    pow_minus_1(63, mat.count() - 1) * 10 * 2
}

// instead of 64**4 get 64*63*62*61
#[inline]
const fn pow_minus_1(exp: u64, left: usize) -> u64 {
    if left > 0 {
        exp * pow_minus_1(exp - 1, left - 1)
    } else {
        1
    }
}

fn to_chess_with_illegal_checks(setup: Setup) -> Result<Chess, PositionError<Chess>> {
    Chess::from_setup(setup, CastlingMode::Standard).or_else(|x| x.ignore_impossible_check())
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            desired_outcome_pos_to_process: VecDeque::new(),
            losing_pos_to_process: VecDeque::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retroboard::shakmaty::fen::Fen;

    #[test]
    fn test_a1_h8_bb() {
        assert_eq!(A1_H1_H8, Bitboard(9277662557957324543))
    }

    #[test]
    fn test_pow_minus_1() {
        assert_eq!(pow_minus_1(64, 1), 64);
        assert_eq!(pow_minus_1(64, 2), 64 * 63);
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

    // #[test]
    // fn test_side_to_move_getter() {
    //     let fen = "4k3/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQ - 0 1";
    //     let rboard = RetroBoard::new_no_pockets(fen).unwrap();
    //     let mut chess: Chess = Fen::from_ascii(fen.as_bytes())
    //         .unwrap()
    //         .into_position(Standard)
    //         .unwrap();
    //     let mut bc = ByColor {
    //         white: 10,
    //         black: 0,
    //     };
    //     assert_eq!(bc.get_by_pos(&rboard), 10);
    //     assert_eq!(bc.get_by_pos(&chess), 10);
    //     chess = chess.swap_turn().unwrap();
    //     assert_eq!(bc.get_by_pos(&chess), 0);
    //     chess = chess.swap_turn().unwrap();
    //     bc.set_to(&chess, 200);
    //     assert_eq!(bc.get_by_pos(&rboard), 200);
    // }

    #[test]
    fn test_ord_outcome() {
        assert!(Outcome::Win(1) > Outcome::Win(2));
        assert!(Outcome::Win(100) > Outcome::Draw);
        assert!(Outcome::Win(100) > Outcome::Lose(1));
        assert!(Outcome::Draw > Outcome::Lose(1));
        assert!(Outcome::Lose(2) > Outcome::Lose(1));
    }

    #[test]
    #[should_panic]
    fn test_ord_outcome_panic() {
        let _ = Outcome::Undefined > Outcome::Win(1);
    }
}
