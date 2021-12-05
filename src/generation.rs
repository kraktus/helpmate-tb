use crate::{TbSetup, index};
use retroboard::RetroBoard;
use shakmaty::{
    Bitboard, CastlingMode::Standard, Color, Color::Black, Color::White, FromSetup, Piece,
    Position, Setup, Square,
};
use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Not};

use indicatif::{ProgressBar, ProgressStyle};

/// According to winnner set in `Generator`
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    Win(u8),
    Draw,
    Lose(u8),
    Unknown,
}

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
    pub winning_pos_to_process: VecDeque<RetroBoard>,
    pub losing_pos_to_process: VecDeque<RetroBoard>,
}

#[derive(Debug, Clone)]
pub struct Generator {
    pub all_pos: HashMap<RetroBoard, Outcome>,
    pub white_king_bb: Bitboard,
    pub winner: Color,
    pub counter: u64,
}

impl Generator {
    fn generate_positions_internal(
        &mut self,
        piece_vec: &[Piece],
        setup: TbSetup,
        queue: &mut Queue,
        pb: &ProgressBar,
    ) {
        match piece_vec {
            [piece, tail @ ..] => {
                //println!("{:?}, setup: {:?}", piece, &setup);
                let squares = if *piece == White.king() {
                    self.white_king_bb
                } else {
                    Bitboard::FULL
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
                    valid_setup.turn = Some(color);
                    self.counter += 1;
                    if self.counter % 100000 == 0 {
                        pb.set_position(self.counter);
                    }
                    if let Ok(chess) = &valid_setup.to_chess_with_illegal_checks() {
                        // if chess is valid then rboard should be too
                        let rboard = RetroBoard::from_setup(&valid_setup, Standard).unwrap();
                        if chess.is_checkmate() {
                            self.all_pos.insert(
                                rboard.clone(),
                                match chess.turn() {
                                    c if c == self.winner => Outcome::Lose(0),
                                    _ => Outcome::Win(0),
                                },
                            );
                            if chess.turn() == self.winner {
                                //println!("lost {:?}", rboard);
                                queue.losing_pos_to_process.push_back(rboard);
                            } else {
                                queue.winning_pos_to_process.push_back(rboard);
                            }
                        } else {
                            self.all_pos.insert(rboard.clone(), Outcome::Draw);
                        }
                    }
                }
            }
        }
    }

    pub fn generate_positions(&mut self, piece_vec: &[Piece], setup: TbSetup) -> Queue {
        let pb = ProgressBar::new(pow_minus_1(63, piece_vec.len()) * 10 * 2);
        pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        // .with_key("eta", |state| format!("{:.1}s", state.eta().as_secs_f64())) only in beta
        .progress_chars("#>-"));
        let mut queue = Queue::default();
        for white_king_sq in self.white_king_bb {
            let mut new_setup = setup.clone();
            new_setup.board.set_piece_at(white_king_sq, White.king());
            self.generate_positions_internal(piece_vec, new_setup, &mut queue, &pb)
        }
        queue
    }

    pub fn process_positions(&mut self, queue: &mut VecDeque<RetroBoard>) {
        loop {
            if let Some(rboard) = queue.pop_front() {
                let out = *self.all_pos.get(&rboard).unwrap();
                for m in rboard.legal_unmoves() {
                    let mut rboard_after_unmove = rboard.clone();
                    rboard_after_unmove.push(&m);
                    match self.all_pos.get(&rboard_after_unmove) {
                        None if self
                            .white_king_bb
                            .contains(rboard_after_unmove.king_of(White)) =>
                        {
                            panic!("pos not found, illegal? {:?}", rboard_after_unmove)
                        }
                        Some(Outcome::Draw) => {
                            queue.push_back(rboard_after_unmove.clone());
                            self.all_pos.insert(rboard_after_unmove, out + 1);
                        }
                        _ => (),
                    }
                    //println!("{:?}", (!out) + 1);
                }
            } else {
                break;
            }
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

impl Default for Generator {
    fn default() -> Self {
        Self {
            all_pos: HashMap::new(),
            white_king_bb: Bitboard::EMPTY // TODO replace that by proper reflection function
                | Square::A1
                | Square::B1
                | Square::C1
                | Square::D1
                | Square::B2
                | Square::C2
                | Square::D2
                | Square::C3
                | Square::D3
                | Square::D4,
            winner: White,
            counter: 0,
        }
    }
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
}
