use shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, Chess, Color, FromSetup, MaterialSide, PositionError,
    RemainingChecks, Setup, Square,
};

use std::num::NonZeroU32;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TbSetup {
    pub board: Board,
    pub ep_square: Option<Square>,
    pub turn: Option<Color>,
}

impl TbSetup {
    pub fn to_chess_with_illegal_checks(&self) -> Result<Chess, PositionError<Chess>> {
        match Chess::from_setup(self, CastlingMode::Standard) {
            Err(x) => x.ignore_impossible_check(),
            pos => pos,
        }
    }
}

impl Setup for TbSetup {
    fn board(&self) -> &Board {
        &self.board
    }
    fn promoted(&self) -> Bitboard {
        Bitboard::EMPTY
    }
    fn pockets(&self) -> Option<&ByColor<MaterialSide>> {
        None
    }
    fn turn(&self) -> Color {
        self.turn.unwrap()
    }
    fn castling_rights(&self) -> Bitboard {
        Bitboard::EMPTY
    }
    fn ep_square(&self) -> Option<Square> {
        self.ep_square
    }
    fn remaining_checks(&self) -> Option<&ByColor<RemainingChecks>> {
        None
    }
    fn halfmoves(&self) -> u32 {
        0
    }
    fn fullmoves(&self) -> NonZeroU32 {
        NonZeroU32::new(1).unwrap()
    }
}

impl Default for TbSetup {
    fn default() -> Self {
        Self {
            board: Board::empty(),
            ep_square: None,
            turn: None,
        }
    }
}
