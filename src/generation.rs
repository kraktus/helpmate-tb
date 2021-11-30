use crate::TbSetup;
use retroboard::RetroBoard;
use shakmaty::{
    CastlingMode::Standard, Chess, Color, Color::Black, Color::White, FromSetup, Piece, Position,
    Setup, Square,
};
use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Not};

/// According to side to move
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
enum Outcome {
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
struct Generator {
    all_pos: HashMap<RetroBoard, Outcome>,
    pos_to_process: VecDeque<RetroBoard>,
}

impl Generator {
    fn generation(&mut self, piece_vec: &mut Vec<Piece>, setup: TbSetup) {
        if let Some(piece) = piece_vec.pop() {
            let range = if piece == White.king() { 0..10 } else { 0..64 };
            for sq in range.map(Square::new) {
                let mut new_setup = setup.clone();
                new_setup.board.set_piece_at(sq, piece);
                self.generation(piece_vec, new_setup);
            }
        } else {
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

    fn process_positions(&mut self) {
        if let Some(rboard) = self.pos_to_process.pop_front() {
        	let out = *self.all_pos.get(&rboard).unwrap();
            for m in rboard.legal_unmoves() {
                let mut rboard_after_unmove = rboard.clone();
                rboard_after_unmove.push(&m);
                if self.all_pos.get(&rboard_after_unmove).is_none() {
                	println!("pos not found, illegal? {:?}", rboard_after_unmove)
                }
                self.all_pos.insert(rboard_after_unmove, (!out) + 1);
            }
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
