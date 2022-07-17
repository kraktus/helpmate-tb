use crate::{index, index_unchecked, restore_from_index, Material, Table, TableBase, A1_H8_DIAG};
use retroboard::RetroBoard;
use shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, CastlingMode::Standard, Chess, Color, Color::Black,
    Color::White, FromSetup, Piece, Position, PositionError, Setup,
};
use std::collections::VecDeque;
use std::ops::{Add, Not};

use std::cmp::{Ord, Ordering, PartialOrd};

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
    fn got(&self, pos: &dyn SideToMove) -> &Self::T;
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T);
}

impl SideToMoveGetter for ByColor<u8> {
    type T = u8;
    fn got(&self, pos: &dyn SideToMove) -> &Self::T {
        self.get(pos.side_to_move())
    }
    fn set_to(&mut self, pos: &dyn SideToMove, t: Self::T) {
        let x_mut = self.get_mut(pos.side_to_move());
        *x_mut = t;
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutcomeOutOfBound;

pub type Outcomes = Vec<ByColor<u8>>;
pub type OutcomesSlice<'a> = &'a [ByColor<u8>];

/// According to winnner set in `Generator`
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    Win(u8), // Need to be between 0 and 125 due to conversion to u8
    Draw,
    Lose(u8), // Need to be between 0 and 125 due to conversion to u8
    Unknown,  // Should we use Option<Outcome> without that variant instead?
}

pub const UNKNOWN_OUTCOME_BYCOLOR: ByColor<u8> = ByColor {
    black: 255,
    white: 255,
};

impl From<u8> for Outcome {
    fn from(u: u8) -> Self {
        match u {
            0 => Self::Draw,
            255 => Self::Unknown,
            w if w >= 128 => Self::Win(w - 128),
            l => Self::Lose(l - 1),
        }
    }
}

impl From<&u8> for Outcome {
    fn from(u: &u8) -> Self {
        (*u).into()
    }
}

impl Ord for Outcome {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Win(x), Self::Win(y)) => x.cmp(y).reverse(), // short win is better,
            (Self::Win(_), Self::Draw | Self::Lose(_)) => Ordering::Greater, // if other is not a Win, we're greater
            (Self::Draw, Self::Win(_)) => Ordering::Less,
            (Self::Draw, Self::Draw) => Ordering::Equal,
            (Self::Draw, Self::Lose(_)) => Ordering::Greater,
            (Self::Lose(x), Self::Lose(y)) => x.cmp(y), // losing in many moves is better,
            (Self::Lose(_), Self::Win(_) | Self::Draw) => Ordering::Less,
            (Self::Unknown, _) | (_, Self::Unknown) => panic!("No unknown in comparison"),
        }
    }
}

impl PartialOrd for Outcome {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn try_into_util(o: Outcome) -> Result<u8, OutcomeOutOfBound> {
    match o {
        Outcome::Draw => Ok(0),
        Outcome::Unknown => Ok(255),
        Outcome::Win(w) if w <= 126 => Ok(w + 128),
        Outcome::Lose(l) if l <= 126 => Ok(l + 1),
        _ => Err(OutcomeOutOfBound),
    }
}

impl From<Outcome> for u8 {
    fn from(o: Outcome) -> Self {
        try_into_util(o).unwrap()
    }
}

impl Not for Outcome {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Win(x) => Self::Lose(x),
            Self::Lose(x) => Self::Win(x),
            Self::Draw => Self::Draw,
            Self::Unknown => Self::Unknown,
        }
    }
}

impl Add<u8> for Outcome {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        match self {
            Self::Win(x) => Self::Win(x + rhs),
            Self::Lose(x) => Self::Lose(x + rhs),
            Self::Draw => Self::Draw,
            Self::Unknown => Self::Unknown,
        }
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
    tablebase: TableBase, // access to the DTM of descendants (different material config, following a capture/promotion)
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
            [] => {
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
                        let rboard = RetroBoard::from_setup(valid_setup, Standard) // DEBUG
                            .expect("if chess is valid then rboard should be too");
                        // let expected_rboard = RetroBoard::new_no_pockets("8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1").unwrap();
                        let idx = index_unchecked(&rboard); // by construction positions generated have white king in the a1-d1-d4 corner
                        let all_pos_idx = self.index_table.encode(&chess);
                        // if rboard.board().kings() == Bitboard::EMPTY | Square::C3 | Square::F3 {
                        //     println!("rboard kings found {rboard:?}, idx: {all_pos_idx:?}");
                        // }
                        //println!("all_pos_idx: {all_pos_idx:?}");
                        // Check that position is generated for the first time/index schema is injective
                        if all_pos_idx == 242414 {
                            println!("Idx: {all_pos_idx:?}, rboard: {rboard:?}");
                        }
                        if Outcome::Unknown != self.all_pos[all_pos_idx].got(&chess).into() {
                            panic!("Index {all_pos_idx} already generated, board: {rboard:?}");
                        }
                        if chess.is_checkmate() {
                            let outcome = match chess.turn() {
                                c if c == self.winner => Outcome::Lose(0),
                                _ => Outcome::Win(0),
                            };
                            self.all_pos[all_pos_idx].set_to(&chess, outcome.into());
                            if chess.turn() == self.winner {
                                //println!("lost {:?}", rboard);
                                queue.losing_pos_to_process.push_back(idx);
                            } else {
                                queue.winning_pos_to_process.push_back(idx);
                            }
                        } else {
                            // println!("{:?}, new idx: {idx}", self.all_pos.get(0).map(|x| x.key()));
                            self.all_pos[all_pos_idx].set_to(&chess, Outcome::Draw.into());
                        }
                    }
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
        self.all_pos = vec![UNKNOWN_OUTCOME_BYCOLOR; self.get_nb_pos() as usize / 10 * 9]; // heuristic, less than 90% of pos are legals. Takes x2 (because each stored element is in fact 1 position, but with black and white to turn) more than number of legal positions
        let white_king_bb = Bitboard(135007759); // a1-d1-d4 triangle
        println!("{:?}", white_king_bb.0);
        for white_king_sq in white_king_bb {
            let mut new_setup = Setup::empty();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(&piece_vec, new_setup, &mut queue, &pb)
        }
        pb.finish_with_message("positions generated");
        println!("all_pos_vec capacity: {}", self.all_pos.capacity());
        while Some(&UNKNOWN_OUTCOME_BYCOLOR) == self.all_pos.last() {
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
                    .into();
                for m in rboard.legal_unmoves() {
                    let mut rboard_after_unmove = rboard.clone();
                    rboard_after_unmove.push(&m);
                    // let chess_after_unmove: Chess = rboard_after_unmove.clone().into();
                    let idx_after_unmove = index(&rboard_after_unmove);
                    let idx_all_pos_after_unmove = self.index_table.encode(&rboard_after_unmove);
                    match self
                        .all_pos
                        .get(idx_all_pos_after_unmove)
                        .map(|bc| bc.got(&rboard_after_unmove))
                    {
                        None => {
                            panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                        }
                        Some(outcome_u8) if Outcome::Draw == outcome_u8.into() => {
                            queue.push_back(idx_after_unmove);
                            self.all_pos[idx_all_pos_after_unmove]
                                .set_to(&rboard_after_unmove, (out + 1).into());
                        }
                        Some(outcome_u8) if Outcome::Unknown == outcome_u8.into() => {
                            panic!("pos before: {rboard:?}, and after {m:?} pos not found, illegal? {rboard_after_unmove:?}, idx: {idx_all_pos_after_unmove:?}")
                        }
                        _ => (),
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
    fn test_outcome_to_u8() {
        assert_eq!(u8::try_from(Outcome::Draw).unwrap(), 0);
        assert_eq!(u8::try_from(Outcome::Unknown).unwrap(), 255);
        assert_eq!(u8::try_from(Outcome::Lose(0)).unwrap(), 1);
        assert_eq!(u8::try_from(Outcome::Lose(125)).unwrap(), 126);
    }

    #[test]
    fn test_u8_to_outcome() {
        for i in 0..u8::MAX {
            assert_eq!(u8::try_from(Outcome::from(i)).unwrap(), i)
        }
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
}
