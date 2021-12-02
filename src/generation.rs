use crate::TbSetup;
use retroboard::RetroBoard;
use shakmaty::{
    Bitboard, CastlingMode::Standard, Color, Color::Black, Color::White, FromSetup, Piece,
    Position, Setup, Square,
};
use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Not};

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
}

impl Generator {
    pub fn generate_positions(&mut self, piece_vec: &[Piece], setup: TbSetup, queue: &mut Queue) {
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
                        self.generate_positions(tail, new_setup, queue);
                    }
                    //println!("after {:?}", &new_setup);
                }
            }
            [] => {
                // setup is complete, check if valid
                for color in [Black, White] {
                    let mut valid_setup = setup.clone();
                    valid_setup.turn = Some(color);
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
                                queue.losing_pos_to_process.push_back(rboard);
                            } else {
                                queue.winning_pos_to_process.push_back(rboard);
                            }
                        } else {
                            self.all_pos.insert(rboard.clone(), Outcome::Unknown);
                        }
                    }
                }
            }
        }
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
                        Some(Outcome::Unknown) => queue.push_back(rboard_after_unmove.clone()),
                        _ => (),
                    }
                    //println!("{:?}", (!out) + 1);
                    self.all_pos.insert(rboard_after_unmove, out + 1); //relative to the player to move
                }
            } else {
                break;
            }
        }
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
    fn test_process_positions_overflow() {
        let mut gen = Generator::default();
        let r =
            RetroBoard::new_no_pockets("k1b5/1p1p4/1P1P4/8/8/1p1p4/1P1P4/K1B5 w - - 0 1").unwrap();
        gen.all_pos.insert(r.clone(), Outcome::Draw);
        gen.pos_to_process.push_back(r.clone());
        gen.process_positions();
    }
}
