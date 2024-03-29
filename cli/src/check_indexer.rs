//! Custom target that perform sanity checks and stats on the indexer
//! Given a material config, it ouputs:
//! - The maximum index for the config
//! - All the positions which are the same modulo symetry, but yield different indexes
//! Run with `cargo tb check-indexer`

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use clap::Args;

use from_str_sequential::FromStrSequential;
use itertools::Itertools;
use log::{debug, info, warn};
use retroboard::{
    shakmaty::{Bitboard, Board, Chess, Color, Color::*, Position, Setup, Square},
    RetroBoard,
};

use helpmate_tb::{
    to_chess_with_illegal_checks, Common, Descendants, Generator, IndexWithTurn, Indexer, Material,
    MaterialWinner, NaiveIndexer, PosHandler, Table,
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
struct CheckIndexerPosHandler {
    // key is the canonical index, and the `Vec` contain all
    // key is only added when at least one duplicate is found
    duplicate_indexes: HashMap<usize, HashSet<usize>>,
    max_index: usize,
}

impl<I: Indexer> PosHandler<I> for CheckIndexerPosHandler {
    fn handle_position(
        &mut self,
        common: &mut Common<I>,
        _: &Descendants,
        chess: &Chess,
        _: IndexWithTurn,
        all_pos_idx: usize,
    ) {
        self.max_index = std::cmp::max(self.max_index, all_pos_idx);
        for transfo in ALL_TRANSFO {
            let transformed_pos = transformed_chess(chess, transfo);
            let transformed_all_pos_idx = common.indexer().encode(&transformed_pos).usize();
            if transformed_all_pos_idx != all_pos_idx {
                debug!(
                    "canonical board: {:?}, idx: {all_pos_idx}",
                    RetroBoard::from(chess.clone())
                );
                debug!(
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

#[derive(Debug, Clone, FromStrSequential)]
pub enum MatOrNbPieces {
    Mat(Material),
    UpTo(usize),
}

impl MatOrNbPieces {
    pub fn materials(&self) -> Vec<Material> {
        match self {
            MatOrNbPieces::Mat(mat) => vec![mat.clone()],
            MatOrNbPieces::UpTo(up_to) => gen_all_pawnless_mat_up_to(*up_to),
        }
    }

    pub fn list_of_materials_with_recursive(&self, recursive: bool) -> Vec<Material> {
        match self {
            MatOrNbPieces::Mat(mat) => {
                let mut materials = if recursive {
                    mat.descendants_recursive(false)
                } else {
                    vec![]
                };
                materials.push(mat.clone());
                materials
            }
            MatOrNbPieces::UpTo(_) => {
                assert!(
                    !recursive,
                    "--recursive not compatible with generating all materials"
                );
                self.materials()
            }
        }
    }
}

#[derive(Debug, Clone, FromStrSequential)]
enum CliIndexer {
    Naive,
    Syzygy,
}

/// Custom target that perform sanity checks and stats on the indexer
/// Given a material config, it ouputs:
/// - The maximum index for the config
/// - All the positions which are the same modulo symetry, but yield different indexes
#[derive(Args, Debug)]
pub struct CheckIndexer {
    #[arg(
        value_parser = MatOrNbPieces::from_str_sequential,
        help = "maximum number of pieces on the board, will check all pawnless material config up to this number included.\nOr just a particular material configuration"
    )]
    mat_or_nb_pieces: MatOrNbPieces,

    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 3)]
    verbose: u8,
    #[arg(long, default_value = "table/")]
    tb_dir: PathBuf,
    #[arg(short, long, default_value = "naive", value_parser = CliIndexer::from_str_sequential)]
    indexer: CliIndexer,
}

fn gen_all_pawnless_mat_up_to(nb_pieces: usize) -> Vec<Material> {
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

    // the two kings must always be on the board
    let mut materials_up_to: Vec<Material> = (1..nb_pieces - 1)
        .flat_map(|i| iter_pieces.clone().combinations(i))
        .map(|pieces_vec| {
            Material::from_iter(pieces_vec.into_iter().chain([Black.king(), White.king()]))
        })
        .collect();
    materials_up_to.sort();
    materials_up_to.dedup(); // dedup only work as expected when the vec is already sorted
    materials_up_to
}

macro_rules! check_index {
    ($indexer:ty, $suffix:tt) => {
        paste::paste! {
        fn [<check_mat_ $suffix>](&self, mat: Material) {
            info!("looking at {mat:?}");
            let mat_win = MaterialWinner::new(&mat, Color::White);
            let mut gen: Generator<CheckIndexerPosHandler, $indexer> = Generator::new_with_pos_handler(
                CheckIndexerPosHandler::default(),
                mat_win,
                &self.tb_dir,
            );
            gen.generate_positions();
            let (_, syzygy_res) = gen.get_result();
            if !syzygy_res.duplicate_indexes.is_empty() {
                warn!(
                    "For {:?}, Found {:?} duplicates",
                    mat,
                    syzygy_res.duplicate_indexes.len()
                );
            }
            info!("Max index is {:?}", syzygy_res.max_index);
        }
        }
    };
}

impl CheckIndexer {
    pub fn run(&self) {
        let all_mats_config = self.mat_or_nb_pieces.materials();
        all_mats_config
            .into_iter()
            .for_each(|mat| match self.indexer {
                CliIndexer::Naive => self.check_mat_naive(mat),
                CliIndexer::Syzygy => self.check_mat_syzygy(mat),
            })
    }

    check_index! {NaiveIndexer, "naive"}
    check_index! {Table, "syzygy"}
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
    .expect("illegal position after applying symetry")
}

#[cfg(test)]
mod tests {

    use super::*;

    // #[test]
    // fn test_known_syzygy_index_duplicate() {
    //     let mut syzygy_check = CheckIndexerPosHandler::default();
    //     let chess: Chess = Fen::from_ascii(b"8/8/2B5/3N4/8/2K2k2/8/8 w - - 0 1")
    //         .unwrap()
    //         .into_position(CastlingMode::Chess960)
    //         .unwrap();
    //     let mut common = Common::new(Material::from_str("KBNvK").unwrap(), Color::White);
    //     let all_pos_idx = common.indexer().encode(&chess).usize();
    //     syzygy_check.handle_position(
    //         &mut common,
    //         &mut Queue::default(),
    //         &Descendants::empty(),
    //         &chess,
    //         IndexWithTurn {
    //             idx: 0,
    //             turn: Color::White,
    //         }, // not used
    //         all_pos_idx,
    //     );
    //     assert_eq!(syzygy_check.max_index, 1907795);
    //     assert_eq!(
    //         syzygy_check.duplicate_indexes,
    //         [(1907795, [1907815].into())].into()
    //     );
    // }

    #[test]
    fn test_gen_all_pawnless_mat_up_to() {
        assert_eq!(gen_all_pawnless_mat_up_to(3).len(), 4);
        assert_eq!(gen_all_pawnless_mat_up_to(4).len(), 24); // 20 4 pieces + 4 3pieces
    }
}
