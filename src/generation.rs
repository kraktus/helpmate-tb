use crate::TbSetup;
use retroboard::RetroBoard;
use shakmaty::{
    CastlingMode::Standard, Color::Black, Color::White, FromSetup, Piece, Position, Setup, Square,
};
use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Not};

/// According to side to move
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    Win(u8),
    Draw,
    Lose(u8),
}

impl Not for Outcome {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Outcome::Win(x) => Outcome::Lose(x),
            Outcome::Lose(x) => Outcome::Win(x),
            Outcome::Draw => Outcome::Draw,
        }
    }
}

impl Add<u8> for Outcome {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        match self {
            Outcome::Win(x) => Outcome::Lose(x + rhs),
            Outcome::Lose(x) => Outcome::Win(x + rhs),
            Outcome::Draw => Outcome::Draw,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Generator {
    pub all_pos: HashMap<RetroBoard, Outcome>,
    pub pos_to_process: VecDeque<RetroBoard>,
}

impl Generator {
    pub fn generate_positions(&mut self, piece_vec: &[Piece], setup: TbSetup) {
        match piece_vec {
            [piece, tail @ ..] => {
                //println!("{:?}, setup: {:?}", piece, &setup);
                let range = if *piece == White.king() { 0..10 } else { 0..64 };
                for sq in range.map(Square::new) {
                    //println!("before {:?}", &setup);
                    if setup.board.piece_at(sq).is_none() {
                        let mut new_setup = setup.clone();
                        new_setup.board.set_piece_at(sq, *piece);
                        self.generate_positions(tail, new_setup);
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
                            self.all_pos.insert(rboard.clone(), Outcome::Lose(0));
                            self.pos_to_process.push_back(rboard);
                        } else {
                            self.all_pos.insert(rboard.clone(), Outcome::Draw);
                        }
                    }
                }
            }
        }
    }

    pub fn process_positions(&mut self) {
        if let Some(rboard) = self.pos_to_process.pop_front() {
            let out = *self.all_pos.get(&rboard).unwrap();
            for m in rboard.legal_unmoves() {
                let mut rboard_after_unmove = rboard.clone();
                rboard_after_unmove.push(&m);
                match self.all_pos.get(&rboard_after_unmove) {
                    None => panic!("pos not found, illegal? {:?}", rboard_after_unmove),
                    Some(Outcome::Draw) => self.pos_to_process.push_back(rboard_after_unmove.clone()),
                    _ =>(),
                }
                self.all_pos.insert(rboard_after_unmove, (!out) + 1); //relative to player to move
            }
            return self.process_positions();
        }
    }
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            all_pos: HashMap::new(),
            pos_to_process: VecDeque::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_positions_overflow() {

    }

}
