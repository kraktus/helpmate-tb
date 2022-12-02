/// Naive indexer compated to `indexer_syzygy`
/// It only handles mapping the white king to the `A1_D1_D4` triangle and then hardcoding the 462 positions two kings
/// can have.
/// It has the benefit of being fast and easily reversible
use retroboard::shakmaty::{
    Bitboard, Board, ByColor, CastlingMode, Color, Color::Black, Color::White, FromSetup, Piece,
    Role, Setup, Square,
};

use crate::{
    generation::{IndexWithTurn, WithBoard},
    indexer_syzygy::{INV_TRIANGLE, KK_IDX, TRIANGLE, Z0},
    is_black_stronger, Material, SideToMove, A1_H8_DIAG,
};
use retroboard::RetroBoard;

pub const A1_D1_D4: Bitboard = Bitboard(135_007_759);

pub const PIECES_ORDER: [Piece; 12] = [
    // kings first
    White.king(),
    Black.king(),
    // then all white pieces
    White.pawn(),
    White.knight(),
    White.bishop(),
    White.rook(),
    White.queen(),
    // finally all black pieces
    Black.pawn(),
    Black.knight(),
    Black.bishop(),
    Black.rook(),
    Black.queen(),
];

// impossible king square setup because by construction the white king
// should be in the A1_D1_D4 triangle
const IMPOSSIBLE_KING_SQ: ByColor<Square> = ByColor {
    white: Square::H8,
    black: Square::H8,
};

const fn invert_kk_idx(kk_idx: &[[u64; 64]; 10]) -> [ByColor<Square>; 462] {
    let mut res: [ByColor<Square>; 462] = [IMPOSSIBLE_KING_SQ; 462];
    let mut white_king_sq: usize = 0;
    loop {
        // for loops not available in const context
        let mut black_king_sq: usize = 0;
        loop {
            let idx = kk_idx[white_king_sq as usize][black_king_sq];
            if idx != Z0 {
                res[idx as usize] = ByColor {
                    white: Square::new(INV_TRIANGLE[white_king_sq] as u32),
                    black: Square::new(black_king_sq as u32),
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

const INV_KK_IDX: [ByColor<Square>; 462] = invert_kk_idx(&KK_IDX);

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

pub trait Indexer {
    fn encode_board_unchecked(&self, b: &Board) -> u64;
    fn encode_board(&self, b: &Board) -> u64;
    fn encode(&self, b: &impl SideToMove) -> IndexWithTurn {
        IndexWithTurn {
            idx: self.encode_board(b.board()),
            turn: b.side_to_move(),
        }
    }
    fn encode_unchecked(&self, b: &impl SideToMove) -> IndexWithTurn {
        IndexWithTurn {
            idx: self.encode_board_unchecked(b.board()),
            turn: b.side_to_move(),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub trait DeIndexer {
    fn restore_board(&self, material: &Material, index: u64) -> Board;
    fn restore(&self, material: &Material, idx_with_turn: IndexWithTurn) -> RetroBoard {
        let mut setup = Setup::empty();
        setup.board = self.restore_board(material, idx_with_turn.idx);
        setup.turn = idx_with_turn.turn;
        RetroBoard::from_setup(setup, CastlingMode::Standard).expect("Right setup")
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NaiveIndexer;

impl From<Material> for NaiveIndexer {
    fn from(_: Material) -> Self {
        Self
    }
}

// should take any board and return the canonical version of it, along with a boolean
// whose truthness is equal to the fact that black were stronger in the original board
pub fn handle_symetry(b: &Board) -> (Board, bool) {
    let mut board_check = b.clone();
    let is_black_stronger = is_black_stronger(b.board());
    if is_black_stronger {
        board_check = swap_color_board(board_check)
    }
    let white_king_sq = board_check.king_of(White).expect("white king");
    let board_transfo_needed = WHITE_KING_SQUARES_TO_TRANSFO[white_king_sq as usize];

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

    for piece in PIECES_ORDER {
        // we check if flipping would result in a "lower" bitboard
        // dictionary order for all their square.
        // This is a better way to check if there is a symetry on the A1_H8 diagonal
        if board_check.by_piece(piece).flip_diagonal() < board_check.by_piece(piece) {
            board_check.flip_diagonal();
            break;
        } else if !A1_H8_DIAG.is_superset(board_check.by_piece(piece)) {
            break;
        }
    }
    (board_check, is_black_stronger)
}

impl Indexer for NaiveIndexer {
    fn encode_board(&self, b: &Board) -> u64 {
        let (board_check, _) = handle_symetry(b);
        self.encode_board_unchecked(&board_check)
    }

    /// ASSUME the white king is in the a1-d1-d4 corner already
    /// If the white king is on the `A1_H8` diagonal, the black king MUST BE in the `A1_H1_H8` triangle
    /// Do not take the turn into account the turn
    fn encode_board_unchecked(&self, b: &Board) -> u64 {
        let mut idx = KK_IDX
            [TRIANGLE[b.board().king_of(White).expect("white king") as usize] as usize]
            [b.board().king_of(Black).expect("black king") as usize];
        debug_assert!(
            idx < 462,
            "Corrupted KK index, board: {:?}, idx: {}",
            b.board(),
            idx
        );
        for role in [
            Role::Pawn,
            Role::Knight,
            Role::Bishop,
            Role::Rook,
            Role::Queen,
        ] {
            for color in Color::ALL {
                for sq in b.board().by_piece(Piece { color, role }) {
                    idx *= 64;
                    idx += sq as u64;
                }
            }
        }
        idx
    }
}

impl DeIndexer for NaiveIndexer {
    fn restore_board(&self, material: &Material, index: u64) -> Board {
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
                let piece = Piece { color, role };
                for _ in 0..material.by_piece(piece) {
                    board.set_piece_at(unsafe { Square::new_unchecked((idx % 64) as u32) }, piece);
                    idx /= 64;
                }
            }
        }
        debug_assert!(idx < 462, "Corrupted index: {index}");
        let kings_sq = INV_KK_IDX[idx as usize];
        board.set_piece_at(kings_sq.black, Black.king());
        board.set_piece_at(kings_sq.white, White.king());
        board
    }
}

/// flip color of pieces and their positions vertically
fn swap_color_board(b: Board) -> Board {
    let (by_roles, by_color) = b.into_bitboards();
    let by_roles_inverted_180 = by_roles.map(Bitboard::flip_vertical);
    Board::from_bitboards(
        by_roles_inverted_180,
        by_color.map(Bitboard::flip_vertical).into_flipped(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use retroboard::shakmaty::{Bitboard, Board};
    use std::num::NonZeroU32;
    use std::str::FromStr;

    #[test]
    fn test_inv_king_idx() {
        for bc in INV_KK_IDX {
            assert!(A1_D1_D4.contains(bc.white))
        }
    }

    fn mat(fen: &str) -> Material {
        Material::from_str(fen).expect("valid fen config to init Material")
    }

    #[test]
    fn test_swap_color_board() {
        let b = Board::from_ascii_board_fen(b"8/8/2p2P2/3nN3/3Bb3/2R2r2/1Q4q1/K6k").unwrap();
        let swapped_b =
            Board::from_ascii_board_fen(b"k6K/1q4Q1/2r2R2/3bB3/3Nn3/2P2p2/8/8").unwrap();
        assert_eq!(swap_color_board(b), swapped_b);
    }

    #[test]
    fn test_index_unchecked_high_value_index() {
        let high_value_board = RetroBoard::new_no_pockets("3BNQQk/8/8/8/3K4/8/8/8 b - -").unwrap();
        let idx = NaiveIndexer.encode_unchecked(&high_value_board);
        let config = mat("KBNQQvK");
        let high_value_from_idx = NaiveIndexer.restore_board(&config, idx.idx);
        assert_eq!(high_value_board.board(), &high_value_from_idx);
    }

    #[test]
    fn test_index_unchecked_then_de_index() {
        let two_kings = RetroBoard::new_no_pockets("8/7k/8/8/3K4/8/8/8 b").unwrap();
        let idx = NaiveIndexer.encode_unchecked(&two_kings);
        let config = mat("KvK");
        let two_kings_from_idx = NaiveIndexer.restore_board(&config, idx.idx);
        assert_eq!(two_kings.board(), &two_kings_from_idx);
    }

    #[test]
    fn test_check_a1_h8_diagonal_symetry() {
        for fen in ["8/8/8/8/8/1QK5/8/k7", "8/8/8/8/8/2K5/2Q5/k7"] {
            let r = Board::from_ascii_board_fen(fen.as_bytes()).unwrap();
            let idx = NaiveIndexer.encode_board(&r);
            assert_eq!(idx, 28938);
        }
    }

    #[test]
    fn test_check_a1_h8_diagonal_symetry2() {
        for fen in ["8/8/8/8/8/2K5/k1Q5/8", "8/8/8/8/8/1QK5/8/1k6"] {
            let r = Board::from_ascii_board_fen(fen.as_bytes()).unwrap();
            let idx = NaiveIndexer.encode_board(&r);
            assert_eq!(idx, 25041);
        }
    }

    #[test]
    fn test_check_a1_h8_diagonal_symetry3() {
        for fen in ["5R1k/7R/8/8/8/8/8/K7", "6Rk/8/7R/8/8/8/8/K7"] {
            let r = Board::from_ascii_board_fen(fen.as_bytes()).unwrap();
            let idx = NaiveIndexer.encode_board(&r);
            assert_eq!(idx, 1_830_397);
        }
    }

    #[test]
    fn test_check_a1_h8_diagonal_symetry4() {
        for fen in ["5R1k/7R/8/8/8/8/8/K7", "6Rk/8/7R/8/8/8/8/K7"] {
            let r = Board::from_ascii_board_fen(fen.as_bytes()).unwrap();
            let idx = NaiveIndexer.encode_board(&r);
            assert_eq!(idx, 1_830_397);
        }
    }

    #[test]
    fn test_check_a1_h8_diagonal_symetry5() {
        for fen in ["5RRk/7R/8/8/8/8/8/K7", "6Rk/7R/7R/8/8/8/8/K7"] {
            let r = Board::from_ascii_board_fen(fen.as_bytes()).unwrap();
            let idx = NaiveIndexer.encode_board(&r);
            assert_eq!(idx, 117_112_318);
        }
    }

    #[test]
    fn test_index_unchecked_then_de_index_no_swapping_color() {
        // check if the color of the pieces are not swapped.
        let knights = RetroBoard::new_no_pockets("8/8/8/8/8/1N6/8/KBkn4 b").unwrap();
        let knights_color_swapped = RetroBoard::new_no_pockets("8/8/8/8/8/1n6/8/KBkN4 b").unwrap();
        let idx = NaiveIndexer.encode_unchecked(&knights);
        let idx_swapped = NaiveIndexer.encode_unchecked(&knights_color_swapped);
        assert_ne!(idx, idx_swapped);
        let config = mat("KBNvKN");
        let knights_from_idx = NaiveIndexer.restore_board(&config, idx.idx);
        let knights_swapped_from_idx = NaiveIndexer.restore_board(&config, idx_swapped.idx);
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
            let idx = NaiveIndexer.encode(&rboard);
            let config = mat("KvK");
            let rboard_restored = NaiveIndexer.restore_board(&config, idx.idx);
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
