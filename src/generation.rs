use crate::TbSetup;
use retroboard::RetroBoard;
use shakmaty::{
    CastlingMode::Standard, Chess, Color, Color::Black, Color::White, FromSetup, Piece, Position,
    Square,
};
use std::collections::{HashMap, VecDeque};

/// According to side to move
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
enum Outcome {
    Win(u8),
    Draw,
    Lose(u8),
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
                if let Ok(chess) = Chess::from_setup(&valid_setup, Standard) {
                    // if chess is valid then rboard should be too
                    let rboard = RetroBoard::from_setup(&valid_setup, Standard).unwrap();
                    if chess.is_checkmate() {
                        self.all_pos.insert(rboard.clone(), Outcome::Lose(0));
                        self.pos_to_process.push_back(rboard);
                    } else {
                        self.all_pos.insert(rboard.clone(), Outcome::Draw);
                    }
                }
            };
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
