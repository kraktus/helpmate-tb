/// Naive indexer compated to `indexer_syzygy`
/// It only handles mapping the white king to the A1_D1_D4 triangle and then hardcoding the 462 positions two kings
/// can have.
/// It has the benefit of being fast and easily reversible
use retroboard::shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, Color, Color::Black, Color::White, FromSetup, Piece,
    Role, Setup, Square,
};

use crate::{
    indexer_syzygy::{KK_IDX, Z0, TRIANGLE},
    Material, generation::WithBoard,
};
use retroboard::RetroBoard;

pub const A1_D1_D4: Bitboard = Bitboard(135007759);

// impossible king square setup because by construction the white king
// should be in the A1_D1_D4 triangle
const IMPOSSIBLE_KING_SQ: ByColor<Square> = ByColor {
    white: Square::H8,
    black: Square::H8,
};

const fn invert_kk_idx(kk_idx: [[u64; 64]; 10]) -> [ByColor<Square>; 462] {
    let mut res: [ByColor<Square>; 462] = [IMPOSSIBLE_KING_SQ; 462];
    let mut white_king_sq: u32 = 0;
    loop {
        // for loops not available in const context
        let mut black_king_sq: u32 = 0;
        loop {
            let idx = kk_idx[white_king_sq as usize][black_king_sq as usize];
            if idx != Z0 {
                res[idx as usize] = ByColor {
                    white: WHITE_KING_INDEX_TO_SQUARE[white_king_sq as usize],
                    black: Square::new(black_king_sq),
                }
            }

            // simulating for 0..64
            black_king_sq += 1;
            if black_king_sq == 64 {
                break;
            }
        }
        // simulating for 0..10
        white_king_sq += 1;
        if white_king_sq == 10 {
            break;
        }
    }
    res
}

const INV_KK_IDX: [ByColor<Square>; 462] = invert_kk_idx(KK_IDX);

#[rustfmt::skip]
const WHITE_KING_SQUARES_TO_INDEX: [usize; 32] = [
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

pub fn index(b: &impl WithBoard) -> u64 {
    let mut board_check = b.board().clone();
    let board_transfo_needed =
        WHITE_KING_SQUARES_TO_TRANSFO[b.board().king_of(White).expect("white king") as usize];
    match board_transfo_needed {
        0 => (),
        1 => board_check.flip_diagonal(),
        2 => board_check.flip_horizontal(),
        3 => board_check.rotate_90(),
        4 => board_check.rotate_270(),
        5 => board_check.flip_vertical(),
        6 => board_check.rotate_180(),
        7 => board_check.flip_anti_diagonal(),
        _ => unreachable!("Only 7 transformations expected"),
    };
    index_unchecked(&board_check)
}

/// ASSUME the white king is in the a1-d1-d4 corner already
/// Do not take the turn into account the turn
pub fn index_unchecked(b: &impl WithBoard) -> u64 {
    let mut idx = KK_IDX
        [TRIANGLE[b.board().king_of(White).expect("white king") as usize] as usize]
        [b.board().king_of(Black).expect("black king") as usize];
    println!("{idx:?}");
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
                println!("{idx:?}");
            }
        }
    }
    idx
}

// DEBUG now the turn is not taken into account
pub fn restore_from_index(material: &Material, index: u64) -> RetroBoard {
    let mut setup = Setup::empty();
    setup.board = restore_from_index_board(material, index);
    RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Right setup")
}

pub fn restore_from_index_board(material: &Material, index: u64) -> Board {
    let mut idx = index;
    let mut board = Board::empty();
    for role in [
        Role::Queen,
        Role::Rook,
        Role::Bishop,
        Role::Knight,
        Role::Pawn,
    ] {
        for color in [Black, White] {
            let piece = Piece { role, color };
            for _ in 0..material.by_piece(piece) {
                board.set_piece_at(unsafe { Square::new_unchecked((idx % 64) as u32) }, piece);
                idx /= 64;
            }
        }
    }
    assert!(idx < 462);
    let kings_sq = INV_KK_IDX[idx as usize];
    board.set_piece_at(kings_sq.black, Black.king());
    board.set_piece_at(kings_sq.white, White.king());
    board //RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Right setup")
}

#[cfg(test)]
mod tests {
    use super::*;
    use retroboard::shakmaty::{Bitboard, Board};
    use std::num::NonZeroU32;

    #[test]
    fn test_inv_king_idx() {
        for bc in INV_KK_IDX {
            assert!(A1_D1_D4.contains(bc.white))
        }
    }

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

    fn mat(fen: &str) -> Material {
        Material::from_str(fen).expect("valid fen config to init Material")
    }

    #[test]
    fn test_index_unchecked_overflow() {
        let high_value_board = RetroBoard::new_no_pockets("3BNQQk/8/8/8/3K4/8/8/8 b - -").unwrap();
        let idx = index_unchecked(&high_value_board);
        let config = mat("KBNQQvK");
        println!("{config:?}");
        let high_value_from_idx = restore_from_index_board(&config, idx);
        assert_eq!(idx, 21474033534);
        assert_eq!(high_value_board.board(), &high_value_from_idx);
    }

    #[test]
    fn test_index_unchecked_then_de_index() {
        let two_kings = RetroBoard::new_no_pockets("8/7k/8/8/3K4/8/8/8 b").unwrap();
        let idx = index_unchecked(&two_kings);
        let config = mat("KvK");
        let two_kings_from_idx = restore_from_index_board(&config, idx);
        assert_eq!(two_kings.board(), &two_kings_from_idx);
    }

    #[test]
    fn test_index_unchecked_then_de_index_no_swapping_color() {
        // check if the color of the pieces are not swapped.
        let knights = RetroBoard::new_no_pockets("8/8/8/8/8/1N6/8/KBkn4 b").unwrap();
        let knights_color_swapped = RetroBoard::new_no_pockets("8/8/8/8/8/1n6/8/KBkN4 b").unwrap();
        let idx = index_unchecked(&knights);
        let idx_swapped = index_unchecked(&knights_color_swapped);
        assert_ne!(idx, idx_swapped);
        let config = mat("KBNvKN");
        let knights_from_idx = restore_from_index_board(&config, idx);
        let knights_swapped_from_idx = restore_from_index_board(&config, idx_swapped);
        assert_eq!(knights.board(), &knights_from_idx);
        assert_eq!(knights_color_swapped.board(), &knights_swapped_from_idx);
    }

    #[test]
    fn test_index_white_king_in_bound() {
        for sq in Square::ALL {
            let mut board = Board::empty();
            board.set_piece_at(sq, White.king());
            board.set_piece_at(sq.offset(16).unwrap_or(Square::A1), Black.king());
            let setup = Setup {
                board,
                turn: Color::Black,
                ep_square: None,
                castling_rights: Bitboard::EMPTY,
                fullmoves: NonZeroU32::try_from(1).unwrap(),
                halfmoves: 0,
                pockets: None,
                promoted: Bitboard::EMPTY,
                remaining_checks: None,
            };
            let rboard =
                RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Valid setup");
            println!("{rboard:?}");
            let idx = index(&rboard);
            let config = mat("KvK");
            let rboard_restored = restore_from_index_board(&config, idx);
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
            assert!(white_king_bb.contains(rboard_restored.king_of(White).expect("White king")));
        }
    }
}
