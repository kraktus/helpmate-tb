use shakmaty::{Bitboard, Board, ByColor, Color, MaterialSide, RemainingChecks, Setup, Square};

use std::num::NonZeroU32;

struct TbSetup {
    pub board: Board,
    pub ep_square: Option<Square>,
    pub turn: Color,
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
        self.turn
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
