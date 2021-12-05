use shakmaty::{Color, Color::Black, Color::White, Piece, Role, Setup};

#[rustfmt::skip]
const WHITE_KING_SQUARES: [u64; 32] = [
    0, 1, 2, 3, 0, 0, 0, 0,
    0, 4, 5, 6, 0, 0, 0, 0,
    0, 0, 7, 8, 0, 0, 0, 0,
    0, 0, 0, 9, 0, 0, 0, 0,
];

// for now ASSUME the white king is in the a1-d1-d4 corner already
pub fn index(b: &dyn Setup) -> u64 {
    let mut idx: u64 = b.turn() as u64;
    idx *= 10;
    idx += WHITE_KING_SQUARES[b.board().king_of(White).expect("white king") as usize];
    idx *= 64;
    idx += b.board().king_of(Black).expect("black king") as u64;
    for role in [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
    ] {
        // https://docs.rs/shakmaty/0.20.2/src/shakmaty/types.rs.html#43-50
        for color in Color::ALL {
            for sq in b.board().by_piece(Piece { role, color }) {
                idx *= 64;
                idx += sq as u64;
            }
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    use shakmaty::Square;

    #[test]
    fn test_white_king_squares() {
        assert_eq!(WHITE_KING_SQUARES[Square::A1 as usize], 0);
        assert_eq!(WHITE_KING_SQUARES[Square::B1 as usize], 1);
        assert_eq!(WHITE_KING_SQUARES[Square::C1 as usize], 2);
        assert_eq!(WHITE_KING_SQUARES[Square::D1 as usize], 3);
        assert_eq!(WHITE_KING_SQUARES[Square::B2 as usize], 4);
        assert_eq!(WHITE_KING_SQUARES[Square::C2 as usize], 5);
        assert_eq!(WHITE_KING_SQUARES[Square::D2 as usize], 6);
        assert_eq!(WHITE_KING_SQUARES[Square::C3 as usize], 7);
        assert_eq!(WHITE_KING_SQUARES[Square::D3 as usize], 8);
        assert_eq!(WHITE_KING_SQUARES[Square::D4 as usize], 9);
    }
}
