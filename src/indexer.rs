use shakmaty::{Board, Color, Color::Black, Color::White, Material, Piece, Role, Setup, Square};

use crate::TbSetup;
use arrayvec::ArrayVec;

// White king is included by default, so need to add it here
type Config = ArrayVec<Piece, 5>;

#[rustfmt::skip]
const WHITE_KING_SQUARES_TO_INDEX: [u64; 32] = [
    0, 1, 2, 3, 10, 10, 10, 10,
    10, 4, 5, 6, 10, 10, 10, 10,
    10, 10, 7, 8, 10, 10, 10, 10,
    10, 10, 10, 9, 10, 10, 10, 10,
];

const WHITE_KING_INDEX_TO_SQUARE: [Square; 10] = [
    Square::A1,
    Square::B1,
    Square::C1,
    Square::D1,
    Square::B2,
    Square::C2,
    Square::D2,
    Square::C3,
    Square::D3,
    Square::D4,
];

// for now ASSUME the white king is in the a1-d1-d4 corner already
pub fn index(b: &dyn Setup) -> u64 {
    let mut idx: u64 = b.turn() as u64;
    idx *= 10;
    let white_king_idx =
        WHITE_KING_SQUARES_TO_INDEX[b.board().king_of(White).expect("white king") as usize];
    assert!(white_king_idx < 10);
    idx += white_king_idx;
    idx *= 64;
    idx += b.board().king_of(Black).expect("black king") as u64;
    for role in [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
    ] {
        for color in Color::ALL {
            for sq in b.board().by_piece(Piece { role, color }) {
                idx *= 64;
                idx += sq as u64;
            }
        }
    }
    idx
}

pub fn from_material(m: &Material) -> Config {
    let mut config = Config::new();
    for role in [
        Role::Queen,
        Role::Rook,
        Role::Bishop,
        Role::Knight,
        Role::Pawn,
    ] {
        for color in [White, Black] {
            config.push(Piece { role, color })
        }
    }
    config
}

pub fn restore_from_index(config: &Config, index: u64) -> TbSetup {
    let mut idx = index;
    let mut setup = TbSetup::default();
    for &piece in config {
        setup
            .board
            .set_piece_at(unsafe { Square::new_unchecked((idx % 64) as u32) }, piece);
        idx /= 64;
    }
    setup.board.set_piece_at(
        unsafe { Square::new_unchecked((idx % 10) as u32) },
        White.king(),
    );
    idx /= 10;

    setup.turn = Some(Color::from_white(idx == 1));
    setup
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_king_squares_to_index() {
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::A1 as usize], 0);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::B1 as usize], 1);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::C1 as usize], 2);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::D1 as usize], 3);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::B2 as usize], 4);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::C2 as usize], 5);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::D2 as usize], 6);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::C3 as usize], 7);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::D3 as usize], 8);
        assert_eq!(WHITE_KING_SQUARES_TO_INDEX[Square::D4 as usize], 9);
    }

    #[test]
    fn test_white_king_index_to_squares() {
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[0], Square::A1);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[1], Square::B1);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[2], Square::C1);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[3], Square::D1);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[4], Square::B2);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[5], Square::C2);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[6], Square::D2);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[7], Square::C3);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[8], Square::D3);
        assert_eq!(WHITE_KING_INDEX_TO_SQUARE[9], Square::D4);
    }

    fn tb_setup(fen: &str) -> TbSetup {
        let board = fen
            .split(' ')
            .next()
            .map(|s| Board::from_board_fen(s.as_bytes()).unwrap())
            .unwrap();
        let turn = fen
            .split(' ')
            .nth(1)
            .and_then(|s| s.chars().next())
            .and_then(Color::from_char)
            .or(Some(White));
        TbSetup {
            board,
            turn,
            ep_square: None,
        }
    }

    #[test]
    fn test_index_overflow() {
        let two_kings = tb_setup("8/5p2/6k1/8/8/8/Q1RB4/1K6 w");
        let idx = index(&two_kings);
        let mut config = Config::new();
        config.push(White.queen());
        config.push(White.rook());
        config.push(White.bishop());
        config.push(Black.pawn());
        config.push(Black.king());
        let two_kings_from_idx = restore_from_index(&config, idx);
        assert_eq!(two_kings, two_kings_from_idx);
    }

    #[test]
    fn test_index_then_de_index() {
        let two_kings = tb_setup("8/8/6k1/8/8/8/8/1K6 w");
        let idx = index(&two_kings);
        let mut config = Config::new();
        config.push(Black.king());
        let two_kings_from_idx = restore_from_index(&config, idx);
        assert_eq!(two_kings, two_kings_from_idx);
    }
}
