use crate::{from_material, index, index_unchecked, restore_from_index, Material, Table};
use retroboard::RetroBoard;
use shakmaty::{
    Bitboard, CastlingMode, CastlingMode::Standard, Chess, Color, Color::Black, Color::White,
    FromSetup, Piece, Position, PositionError, Setup, Square,
};
use std::collections::VecDeque;
use std::ops::{Add, Not};

use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutcomeOutOfBound;

/// According to winnner set in `Generator`
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    Win(u8), // Need to be between 0 and 125 due to conversion to u8
    Draw,
    Lose(u8), // Need to be between 0 and 125 due to conversion to u8
    Unknown,
}

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

// impl From<TbKeyValue> for Outcome {
//     fn from(key_value: TbKeyValue) -> Self {
//         Outcome::from(u8::try_from(key_value.value()).unwrap())
//     }
// }

// impl From<&TbKeyValue> for Outcome {
//     fn from(key_value: &TbKeyValue) -> Self {
//         Outcome::from(u8::try_from(key_value.value()).unwrap())
//     }
// }

// impl From<Outcome> for u128 {
//     fn from(o: Outcome) -> Self {
//         u8::from(o) as u128
//     }
// }

impl Not for Outcome {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Outcome::Win(x) => Outcome::Lose(x),
            Outcome::Lose(x) => Outcome::Win(x),
            Outcome::Draw => Outcome::Draw,
            Outcome::Unknown => Outcome::Unknown,
        }
    }
}

impl Add<u8> for Outcome {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        match self {
            Outcome::Win(x) => Outcome::Win(x + rhs),
            Outcome::Lose(x) => Outcome::Lose(x + rhs),
            Outcome::Draw => Outcome::Draw,
            Outcome::Unknown => Outcome::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Queue {
    pub winning_pos_to_process: VecDeque<u64>,
    pub losing_pos_to_process: VecDeque<u64>,
}

#[derive(Debug, Clone)]
pub struct Generator {
    pub all_pos: Vec<u8>,
    pub winner: Color,
    pub counter: u64,
    material: Material,
    table: Table,
}

impl Generator {
    fn generate_positions_internal(
        &mut self,
        piece_vec: &[Piece],
        setup: Setup,
        queue: &mut Queue,
        all_pos: &mut Vec<u8>,
        pb: &ProgressBar,
    ) {
        match piece_vec {
            [piece, tail @ ..] => {
                //println!("{:?}, setup: {:?}", piece, &setup);
                let squares = Bitboard::FULL; // white king handled in `generate_positions`
                for sq in squares {
                    //println!("before {:?}", &setup);
                    if setup.board.piece_at(sq).is_none() {
                        let mut new_setup = setup.clone();
                        new_setup.board.set_piece_at(sq, *piece);
                        self.generate_positions_internal(tail, new_setup, queue, all_pos, pb);
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
                        // if chess is valid then rboard should be too
                        let rboard = RetroBoard::from_setup(valid_setup, Standard).unwrap();
                        let idx = index_unchecked(&rboard); // by construction positions generated have white king in the a1-d1-d4 corner
                        let all_pos_idx = self.table.encode(&chess).unwrap();
                        assert!(Outcome::Unknown == all_pos[all_pos_idx].into()); // Check that position is generated for the first time/index schema is injective
                        if chess.is_checkmate() {
                            let outcome = match chess.turn() {
                                c if c == self.winner => Outcome::Lose(0),
                                _ => Outcome::Win(0),
                            };
                            all_pos[all_pos_idx] = outcome.into();
                            if chess.turn() == self.winner {
                                //println!("lost {:?}", rboard);
                                queue.losing_pos_to_process.push_back(idx);
                            } else {
                                queue.winning_pos_to_process.push_back(idx);
                            }
                        } else {
                            // println!("{:?}, new idx: {idx}", self.all_pos.get(0).map(|x| x.key()));
                            all_pos[all_pos_idx] = Outcome::Draw.into();
                        }
                    }
                }
            }
        }
    }

    pub fn generate_positions(&mut self, setup: Setup) -> Queue {
        let piece_vec = from_material(&self.material);
        let pb = self.get_progress_bar();
        self.counter = 0;
        let mut queue = Queue::default();
        let mut all_pos_vec: Vec<u8> = [255].repeat(self.get_nb_pos() as usize / 10 * 9 * 4); // heuristic, less than 90% of pos are legals. Takes x4 more than number of legal positions
        let white_king_bb = Bitboard::EMPTY
            | Square::A1
            | Square::B1
            | Square::C1
            | Square::D1
            | Square::B2
            | Square::C2
            | Square::D2
            | Square::C3
            | Square::D3
            | Square::D4;
        for white_king_sq in white_king_bb {
            let mut new_setup = setup.clone();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(
                &piece_vec,
                new_setup,
                &mut queue,
                &mut all_pos_vec,
                &pb,
            )
        }
        pb.finish_with_message("positions generated");
        println!("all_pos_vec capacity: {}", all_pos_vec.capacity());
        all_pos_vec.shrink_to_fit();
        println!(
            "all_pos_vec capacity: {} after shrinking",
            all_pos_vec.capacity()
        );
        self.all_pos = all_pos_vec.try_into().expect("unique indexes");
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
        pow_minus_1(63, self.material.count()) * 10 * 2
    }

    pub fn process_positions(&mut self, queue: &mut VecDeque<u64>) {
        let config = from_material(&self.material);
        let pb = self.get_progress_bar();
        self.counter = 0;
        loop {
            if let Some(idx) = queue.pop_front() {
                self.counter += 1;
                if self.counter % 100000 == 0 {
                    pb.set_position(self.counter);
                }
                let rboard = restore_from_index(&config, idx);
                let out: Outcome = self
                    .all_pos
                    .get(self.table.encode(&Chess::from(rboard.clone())).unwrap())
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
                    let chess_after_unmove: Chess = rboard_after_unmove.clone().into();
                    let idx_after_unmove = index(&rboard_after_unmove);
                    let idx_all_pos_after_unmove = self.table.encode(&chess_after_unmove).unwrap();
                    match self.all_pos.get(idx_all_pos_after_unmove) {
                        None => {
                            panic!("pos not found, illegal? {:?}", rboard_after_unmove)
                        }
                        Some(outcome_u8) if Outcome::Draw == outcome_u8.into() => {
                            queue.push_back(idx_after_unmove);
                            self.all_pos[idx_all_pos_after_unmove] = (out + 1).into()
                        }
                        Some(outcome_u8) if Outcome::Unknown == outcome_u8.into() => {
                            panic!("pos not found, illegal? {:?}", rboard_after_unmove)
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

    pub fn new(fen_config: &str) -> Self {
        let material = Material::from_str(fen_config).expect("valid fen config to init Material");
        Self {
            all_pos: Vec::default(),
            winner: White,
            counter: 0,
            table: Table::new(material.pieces()),
            material,
        }
    }
}

// instead of 64**4 get 64*63*62*61
#[inline]
const fn pow_minus_1(exp: u64, left: usize) -> u64 {
    if left >= 1 {
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
}
