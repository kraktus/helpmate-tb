use shakmaty::{
    CastlingMode, Color, Color::Black, Color::White, FromSetup, Piece, Role, Setup, Square,
};

use crate::{Material, Pieces};
use retroboard::RetroBoard;

#[rustfmt::skip]
const WHITE_KING_SQUARES_TO_INDEX: [u64; 32] = [
    0,  1, 2, 3, 10, 10, 10, 10,
   10,  4, 5, 6, 10, 10, 10, 10,
   10, 10, 7, 8, 10, 10, 10, 10,
   10, 10,10, 9, 10, 10, 10, 10,
];

#[rustfmt::skip]
const WHITE_KING_SQUARES_TO_TRANSFO: [u64; 64] = [
    0, 0, 0, 0, 2, 2, 2, 2,
    1, 0, 0, 0, 2, 2, 2, 3,
    1, 1, 0, 0, 2, 2, 3, 3,
    1, 1, 1, 0, 2, 3, 3, 3,
    4, 4, 4, 5, 6, 7, 7, 7,
    4, 4, 5, 5, 6, 6, 7, 7,
    4, 5, 5, 5, 6, 6, 6, 7,
    5, 5, 5, 5, 6, 6, 6, 6,
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

pub fn index(b: &RetroBoard) -> u64 {
    let mut rboard_checked = b.clone();
    let board_transfo_needed =
        WHITE_KING_SQUARES_TO_TRANSFO[b.board().king_of(White).expect("white king") as usize];
    match board_transfo_needed {
        0 => (),
        1 => rboard_checked.flip_diagonal(),
        2 => rboard_checked.flip_horizontal(),
        3 => rboard_checked.rotate_90(),
        4 => rboard_checked.rotate_270(),
        5 => rboard_checked.flip_vertical(),
        6 => rboard_checked.rotate_180(),
        7 => rboard_checked.flip_anti_diagonal(),
        _ => panic!("Only 7 transformations expected"),
    };
    index_unchecked(&rboard_checked)
}

/// ASSUME the white king is in the a1-d1-d4 corner already
pub fn index_unchecked(b: &RetroBoard) -> u64 {
    let mut idx: u64 = b.retro_turn() as u64;
    idx *= 10;
    let white_king_idx =
        WHITE_KING_SQUARES_TO_INDEX[b.board().king_of(White).expect("white king") as usize];
    if white_king_idx >= 10 {
        panic!("Wrong king index, retroboard: {:?}", b);
    }
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

pub fn restore_from_index(config: &Pieces, index: u64) -> RetroBoard {
    let mut idx = index;
    let mut setup = Setup::empty();
    for &piece in config {
        setup
            .board
            .set_piece_at(unsafe { Square::new_unchecked((idx % 64) as u32) }, piece);
        idx /= 64;
    }
    setup.board.set_piece_at(
        WHITE_KING_INDEX_TO_SQUARE[(idx % 10) as usize],
        White.king(),
    );
    idx /= 10;

    // index takes as an input a `RetroBoard`, and `retro_turn` == !`turn` so to return the right retro-turn, we need to put the reverse turn.
    setup.turn = Color::from_white(idx == 0);
    RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Right setup")
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{Bitboard, Board};

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

    // fn mat(fen: &str) -> Config {
    //     from_material(&Material::from_ascii_fen(fen.as_bytes()).unwrap())
    // }

    #[test]
    fn test_index_unchecked_overflow() {
        let high_value_board = RetroBoard::new_no_pockets("3bnqqk/8/8/8/3K4/8/8/8 b").unwrap();
        let idx = index_unchecked(&high_value_board);
        let config = mat("bnqqk");
        let high_value_from_idx = restore_from_index(&config, idx);
        //assert_eq!(idx, 21474565947);
        assert_eq!(high_value_board, high_value_from_idx);
    }

    #[test]
    fn test_index_unchecked_then_de_index() {
        let two_kings = RetroBoard::new_no_pockets("8/7k/8/8/3K4/8/8/8 b").unwrap();
        let idx = index_unchecked(&two_kings);
        let config = mat("k");
        let two_kings_from_idx = restore_from_index(&config, idx);
        assert_eq!(two_kings, two_kings_from_idx);
    }

    #[test]
    fn test_index_unchecked_then_de_index_no_swapping_color() {
        // check if the color of the pieces are not swapped.
        let knights = RetroBoard::new_no_pockets("8/8/8/8/8/1N6/8/KBkn4 b").unwrap();
        let knights_color_swapped = RetroBoard::new_no_pockets("8/8/8/8/8/1n6/8/KBkN4 b").unwrap();
        let idx = index_unchecked(&knights);
        let idx_swapped = index_unchecked(&knights_color_swapped);
        assert_ne!(idx, idx_swapped);
        let config = mat("BNnk");
        let knights_from_idx = restore_from_index(&config, idx);
        let knights_swapped_from_idx = restore_from_index(&config, idx_swapped);
        assert_eq!(knights, knights_from_idx);
        assert_eq!(knights_color_swapped, knights_swapped_from_idx);
    }

    #[test]
    fn test_index_white_king_in_bound() {
        for sq in Square::ALL {
            let mut board = Board::empty();
            board.set_piece_at(sq, White.king());
            board.set_piece_at(sq.offset(16).unwrap_or(Square::A1), Black.king());
            let setup = TbSetup {
                board,
                turn: Some(Color::Black),
                ep_square: None,
            };
            let rboard =
                RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Valid setup");
            let idx = index(&rboard);
            let config = mat("k");
            let rboard_restored = restore_from_index(&config, idx);
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
            assert!(white_king_bb.contains(rboard_restored.king_of(White)));
        }
    }
}
