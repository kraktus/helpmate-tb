//! Custom target that perform sanity checks and stats on the syzygy indexer
//! Given a material config, it ouputs:
//! - The maximum index for the config
//! - All the positions which are the same modulo symetry, but yield different indexes
//! Run with `cargo run syzygy-check`

use std::collections::{HashMap, HashSet};

use clap::Parser;
use itertools::Itertools;
use retroboard::{
    shakmaty::{Bitboard, Board, Chess, Color, Color::*, Position, Setup, Square},
    RetroBoard,
};

use crate::{
    to_chess_with_illegal_checks, Common, Descendants, Generator, Material, Pieces, PosHandler,
    Queue,
};

type Transfo = (
    fn(&mut Board),
    fn(Bitboard) -> Bitboard,
    fn(Square) -> Square,
);

// same order as in shakmaty doc
const ALL_TRANSFO: [Transfo; 7] = [
    (
        Board::flip_vertical,
        Bitboard::flip_vertical,
        Square::flip_vertical,
    ),
    (
        Board::flip_horizontal,
        Bitboard::flip_horizontal,
        Square::flip_horizontal,
    ),
    (
        Board::flip_diagonal,
        Bitboard::flip_diagonal,
        Square::flip_diagonal,
    ),
    (
        Board::flip_anti_diagonal,
        Bitboard::flip_anti_diagonal,
        Square::flip_anti_diagonal,
    ),
    (Board::rotate_90, Bitboard::rotate_90, Square::rotate_90),
    (Board::rotate_270, Bitboard::rotate_270, Square::rotate_270),
    (Board::rotate_180, Bitboard::rotate_180, Square::rotate_180),
];

#[derive(Debug, Clone, Default)]
struct SyzygyCheck {
    // key is the canonical index, and the `Vec` contain all
    // key is only added when at least one duplicate is found
    duplicate_indexes: HashMap<usize, HashSet<usize>>,
    max_index: usize,
}

impl PosHandler for SyzygyCheck {
    fn handle_position(
        &mut self,
        common: &mut Common,
        _: &mut Queue,
        _: &Descendants,
        chess: &Chess,
        _: u64,
        all_pos_idx: usize,
    ) {
        self.max_index = std::cmp::max(self.max_index, all_pos_idx);
        for transfo in ALL_TRANSFO {
            let transformed_pos = transformed_chess(chess, transfo);
            let transformed_all_pos_idx = common.index_table().encode(&transformed_pos);
            if transformed_all_pos_idx != all_pos_idx {
                println!(
                    "canonical board: {:?}, idx: {all_pos_idx}",
                    RetroBoard::from(chess.clone())
                );
                println!(
                    "transformed board: {:?}, idx: {transformed_all_pos_idx}",
                    RetroBoard::from(transformed_pos.clone())
                );
                if let Some(set) = self.duplicate_indexes.get_mut(&all_pos_idx) {
                    set.insert(transformed_all_pos_idx);
                } else {
                    self.duplicate_indexes
                        .insert(all_pos_idx, [transformed_all_pos_idx].into());
                }
            }
        }
    }
}

fn check_mat(mat: Material) {
    let common = Common::new(mat, Color::White);
    let mut gen = Generator::new_with_pos_handler(SyzygyCheck::default(), common);
    gen.generate_positions();
    let (_, _, _) = gen.get_result();
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opt {
    #[clap(
        short,
        long,
        value_parser,
        help = "maximum number of pieces on the board, will check all pawnless material config up to this number included"
    )]
    nb_pieces: usize,
}

fn gen_all_pawnless_mat_up_to(nb_pieces: usize) -> HashSet<Material> {
    let iter_pieces = vec![
        Black.bishop(),
        Black.knight(),
        Black.rook(),
        Black.queen(),
        White.bishop(),
        White.knight(),
        White.rook(),
        White.queen(),
    ]
    .repeat(nb_pieces - 2)
    .into_iter();

    (1..nb_pieces - 1)
        .flat_map(|i| iter_pieces.clone().combinations(i))
        .map(|pieces_vec| {
            Material::from_iter(pieces_vec.into_iter().chain([Black.king(), White.king()]))
        })
        .collect()
}

fn main() {
    let args = Opt::parse();
    let all_mats_config = gen_all_pawnless_mat_up_to(args.nb_pieces);
    for mat in all_mats_config {}
}

fn transformed_chess(chess: &Chess, transfo: Transfo) -> Chess {
    let mut board = chess.board().clone();
    (transfo.0)(&mut board);
    to_chess_with_illegal_checks(Setup {
        board,
        promoted: (transfo.1)(chess.promoted()),
        pockets: chess.pockets().copied(),
        turn: chess.turn(),
        castling_rights: (transfo.1)(chess.castles().castling_rights()),
        ep_square: chess.maybe_ep_square().map(transfo.2),
        remaining_checks: chess.remaining_checks().copied(),
        halfmoves: chess.halfmoves(),
        fullmoves: chess.fullmoves(),
    })
    .expect("illegal position after applying sysmetry")
}

#[cfg(test)]
mod tests {
    use retroboard::shakmaty::{fen::Fen, CastlingMode, Color};

    use crate::Material;

    use super::*;

    #[test]
    fn test_known_index_duplicate() {
        let mut syzygy_check = SyzygyCheck::default();
        let chess: Chess = Fen::from_ascii(b"8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1")
            .unwrap()
            .into_position(CastlingMode::Chess960)
            .unwrap();
        let mut common = Common::new(Material::from_str("KBNvK").unwrap(), Color::White);
        let all_pos_idx = common.index_table().encode(&chess);
        syzygy_check.handle_position(
            &mut common,
            &mut Queue::default(),
            &Descendants::empty(),
            &chess,
            0, // not used
            all_pos_idx,
        );
        assert_eq!(syzygy_check.max_index, 1907795);
        assert_eq!(
            syzygy_check.duplicate_indexes,
            [(1907795, [1907815].into())].into()
        );
    }

    #[test]
    fn test_gen_all_pawnless_mat_up_to() {
        assert_eq!(gen_all_pawnless_mat_up_to(3).len(), 4);
        assert_eq!(dbg!(gen_all_pawnless_mat_up_to(4)).len(), 24); // 20 4 pieces + 4 3pieces
    }
}
