use crate::{index, index_unchecked, restore_from_index, Material, Table, TableBase, A1_H8_DIAG};
use crate::{Outcome, Outcomes, Report, UNDEFINED_OUTCOME_BYCOLOR};
use retroboard::RetroBoard;
use shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, CastlingMode::Standard, Chess, Color, Color::Black,
    Color::White, FromSetup, Outcome as ChessOutcome, Piece, Position, PositionError, Setup,
};
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
    // chose `got` and not `get` not to shadow the original methods
    fn got(&self, pos: &dyn SideToMove) -> Self::T;
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T);
}

impl SideToMoveGetter for ByColor<u8> {
    type T = Report;
    fn got(&self, pos: &dyn SideToMove) -> Self::T {
        self.get(pos.side_to_move()).into()
    }
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t.into();
    }
}

#[derive(Debug, Clone)]
pub struct Queue {
    pub winning_pos_to_process: VecDeque<u64>,
    pub losing_pos_to_process: VecDeque<u64>,
}

const A1_H1_H8: Bitboard = Bitboard(0x80c0e0f0f8fcfeff);

#[derive(Debug)]
pub struct Generator {
    pub all_pos: Outcomes,
    pub winner: Color,
    pub counter: u64,
    pub material: Material,
    index_table: Table,
    tablebase: Option<TableBase>, // access to the DTM of descendants (different material config, following a capture/promotion)
}

impl Generator {
    fn generate_positions_internal(
        &mut self,
        piece_vec: &[Piece],
        setup: Setup,
        queue: &mut Queue,
        pb: &ProgressBar,
    ) {
        match piece_vec {
            [piece, tail @ ..] => {
                //println!("{:?}, setup: {:?}", piece, &setup);
                let squares = if A1_H8_DIAG.is_superset(setup.board.occupied()) {
                    A1_H1_H8
                } else {
                    Bitboard::FULL // white king handled in `generate_positions`
                };
                for sq in squares {
                    //println!("before {:?}", &setup);
                    if setup.board.piece_at(sq).is_none() {
                        let mut new_setup = setup.clone();
                        new_setup.board.set_piece_at(sq, *piece);
                        self.generate_positions_internal(tail, new_setup, queue, pb);
                    }
                    //println!("after {:?}", &new_setup);
                }
            }
            [] => Self::generate_positions_check_position(self, setup, queue, pb),
        }
    }

    // TODO split the `Generator` in two `Generator` and `Processor`, not to move around these parameters so much
    fn generate_positions_check_position(
        &mut self,
        setup: Setup,
        queue: &mut Queue,
        pb: &ProgressBar,
    ) {
        // setup is complete, check if valid
        for color in [Black, White] {
            let mut valid_setup = setup.clone();
            valid_setup.turn = color;
            self.counter += 1;
            if self.counter % 100000 == 0 {
                pb.set_position(self.counter);
            }
            // println!("{:?}", valid_setup);
            if let Ok(chess) = to_chess_with_illegal_checks(valid_setup.clone()) {
                let rboard =
                    RetroBoard::from_setup(valid_setup, Standard) // DEBUG
                        .expect("if chess is valid then rboard should be too");
                // let expected_rboard = RetroBoard::new_no_pockets("8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1").unwrap();
                let idx = index_unchecked(&rboard); // by construction positions generated have white king in the a1-d1-d4 corner
                                                    // if the position is a stalemate, index is not unique, must be sorted later
                let all_pos_idx = self.index_table.encode(&chess);
                // if rboard.board().kings() == Bitboard::EMPTY | Square::C3 | Square::F3 {
                //     println!("rboard kings found {rboard:?}, idx: {all_pos_idx:?}");
                // }
                //println!("all_pos_idx: {all_pos_idx:?}");
                // Check that position is generated for the first time/index schema is injective
                if all_pos_idx == 23506 {
                    println!("Idx: {all_pos_idx:?}, rboard: {rboard:?}");
                }
                if Outcome::Undefined != self.all_pos[all_pos_idx].got(&chess).outcome() {
                    panic!("Index {all_pos_idx} already generated, board: {rboard:?}");
                }
                match chess_outcome(&chess) {
                    Some(ChessOutcome::Decisive { winner }) => {
                        // we know the result is exact, since the game is over
                        let outcome = Report::Processed(if winner == self.winner {
                            Outcome::Win(0)
                        } else {
                            Outcome::Lose(0)
                        });
                        self.all_pos[all_pos_idx].set_to(&chess, outcome);
                        if winner == self.winner {
                            //println!("lost {:?}", rboard);
                            queue.losing_pos_to_process.push_back(idx);
                        } else {
                            queue.winning_pos_to_process.push_back(idx);
                        }
                    }
                    None => {
                        // println!("{:?}, new idx: {idx}", self.all_pos.get(0).map(|x| x.key()));
                        self.all_pos[all_pos_idx].set_to(
                            &chess,
                            Report::Unprocessed(
                                self.tablebase
                                    .as_ref()
                                    .and_then(|tb| tb.outcome_from_captures_promotion(&chess))
                                    .unwrap_or(Outcome::Draw),
                            ),
                        );
                    }
                    Some(ChessOutcome::Draw) => (), // Stalemate positions. Nothing to do result is known, and cannot be stored because they induce collision with Syzygy indexer
                }
            }
        }
    }

    pub fn generate_positions(&mut self) -> Queue {
        let piece_vec = self.material.pieces_without_white_king();
        println!("{piece_vec:?}");
        let pb = self.get_progress_bar();
        self.counter = 0;
        let mut queue = Queue::default();
        self.all_pos = vec![UNDEFINED_OUTCOME_BYCOLOR; self.get_nb_pos() as usize / 10 * 9]; // heuristic, less than 90% of pos are legals. Takes x2 (because each stored element is in fact 1 position, but with black and white to turn) more than number of legal positions
        let white_king_bb = Bitboard(135007759); // a1-d1-d4 triangle
        println!("{:?}", white_king_bb.0);
        for white_king_sq in white_king_bb {
            let mut new_setup = Setup::empty();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(&piece_vec, new_setup, &mut queue, &pb)
        }
        pb.finish_with_message("positions generated");
        println!("all_pos_vec capacity: {}", self.all_pos.capacity());
        while Some(&UNDEFINED_OUTCOME_BYCOLOR) == self.all_pos.last() {
            self.all_pos.pop();
        }

        self.all_pos.shrink_to_fit();
        println!(
            "all_pos_vec capacity: {} after shrinking",
            self.all_pos.capacity()
        );
        queue
    }

    fn get_progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(self.get_nb_pos());
        pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));
        pb
    }

    #[inline]
    fn get_nb_pos(&self) -> u64 {
        // white king is already included in `material.count()`, so substract it, and multiply by 10 instead, real number of cases the white king can go on
        pow_minus_1(63, self.material.count() - 1) * 10 * 2
    }

    pub fn process_positions(&mut self, queue: &mut VecDeque<u64>) {
        // let config = self.material.pieces_without_white_king();
        let pb = self.get_progress_bar();
        self.counter = 0;
        loop {
            if let Some(idx) = queue.pop_front() {
                self.counter += 1;
                if self.counter % 100000 == 0 {
                    pb.set_position(self.counter);
                }
                let rboard = restore_from_index(&self.material, idx);
                let out: Outcome = self
                    .all_pos
                    .get(self.index_table.encode(&rboard))
                    .map(|bc| bc.got(&rboard))
                    .unwrap_or_else(|| {
                        panic!(
                            "idx got {}, idx recomputed {}, rboard {:?}",
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
                    let idx_all_pos_after_unmove = self.index_table.encode(&rboard_after_unmove);
                    match self
                        .all_pos
                        .get(idx_all_pos_after_unmove) // TODO use direct index self.all_pos[idx_all_pos_after_unmove]
                        .map(|bc| bc.got(&rboard_after_unmove))
                        .filter(|r| matches!(r, Report::Unprocessed(_)))
                        .map(|r| r.outcome())
                    {
                        None => {
                            panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                        }
                        Some(outcome_u8) if Outcome::Undefined == outcome_u8.into() => {
                            panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                        }
                        Some(fetched_outcome) => {
                            // we know the position is unprocessed
                            queue.push_back(idx_after_unmove);
                            // if the outcome fetched is Draw, it means no result is stored yet
                            let processed_outcome =
                                Report::Processed(if fetched_outcome == Outcome::Draw {
                                    out + 1
                                } else {
                                    // if some actual result is written (because found by a capture/promotion/other position)
                                    // we write the best outcome
                                    (out + 1).max(fetched_outcome)
                                });
                            self.all_pos[idx_all_pos_after_unmove]
                                .set_to(&rboard_after_unmove, processed_outcome);
                        }
                    }
                    //println!("{:?}", (!out) + 1);
                }
            } else {
                break;
            }
        }
        pb.finish_with_message("positions processed");
    }

    pub fn new(material: Material) -> Self {
        Self {
            all_pos: Vec::default(),
            winner: White,
            counter: 0,
            index_table: Table::new(&material),
            tablebase: TableBase::new(&material),
            material,
        }
    }
}

#[inline]
// workaround of shakmaty calling twice legal_moves
// waiting for https://github.com/niklasf/shakmaty/pull/59
fn chess_outcome(chess: &Chess) -> Option<ChessOutcome> {
    if chess.legal_moves().is_empty() {
        if !chess.checkers().is_empty() {
            Some(ChessOutcome::Decisive {
                winner: !chess.turn(),
            })
        } else {
            // stalemate
            Some(ChessOutcome::Draw)
        }
    } else if chess.is_insufficient_material() {
        Some(ChessOutcome::Draw)
    } else {
        None
    }
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
            winning_pos_to_process: VecDeque::new(),
            losing_pos_to_process: VecDeque::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::fen::Fen;

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

    #[test]
    fn test_side_to_move_getter() {
        let fen = "4k3/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQ - 0 1";
        let rboard = RetroBoard::new_no_pockets(fen).unwrap();
        let mut chess: Chess = Fen::from_ascii(fen.as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let mut bc = ByColor {
            white: 10,
            black: 0,
        };
        assert_eq!(*bc.got(&rboard), 10);
        assert_eq!(*bc.got(&chess), 10);
        chess = chess.swap_turn().unwrap();
        assert_eq!(*bc.got(&chess), 0);
        chess = chess.swap_turn().unwrap();
        bc.set_to(&chess, 200);
        assert_eq!(*bc.got(&rboard), 200);
    }

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
