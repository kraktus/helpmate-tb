use std::{collections::HashMap, path::Path};

use itertools::process_results;
use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{ByColor, Chess, Color, MoveList, Position};

use crate::{
    file_handler::RetrieveOutcome, DefaultIndexer, EncoderDecoder, Indexer, Material,
    MaterialWinner, Outcome, SideToMove,
};
use std::io;

#[derive(Debug)]
pub struct LazyFileHandler<T = DefaultIndexer> {
    indexer: T,
    inner: EncoderDecoder<RandomAccessFile>,
}

impl<T: Indexer> LazyFileHandler<T> {
    #[must_use]
    pub fn new(mat: &MaterialWinner, tablebase_dir: &Path) -> Self {
        let path = tablebase_dir.join(format!("{mat:?}"));
        let raf =
            RandomAccessFile::open(&path).unwrap_or_else(|_| panic!("Path {path:?} not found"));
        let inner = EncoderDecoder::new(raf);
        let indexer = T::new(&mat.material);
        Self { inner, indexer }
    }

    #[must_use]
    pub fn outcome_of(&self, board_and_turn: &impl SideToMove) -> io::Result<Outcome> {
        self.inner
            .outcome_of(self.indexer.encode_board(board_and_turn.board()))
            .map(|bc| bc.get(board_and_turn.side_to_move()).clone())
            .map(Outcome::from)
    }
}

#[derive(Debug)]
pub struct TablebaseProber<T = DefaultIndexer>(HashMap<Material, ByColor<LazyFileHandler<T>>>);

impl<T: Indexer> TablebaseProber<T> {
    #[must_use]
    pub fn new(mat: &Material, tablebase_dir: &Path) -> Self {
        let mut mats = mat.descendants_recursive(false);
        mats.push(mat.clone());
        Self(
            mats.into_iter()
                .map(|m| {
                    (
                        m.clone(),
                        ByColor::new_with(|winner| {
                            let mat_winner = MaterialWinner::new(&m, winner);
                            LazyFileHandler::new(&mat_winner, tablebase_dir)
                        }),
                    )
                })
                .collect(),
        )
    }

    /// Returns one of the best possible line until mate or drawn position
    pub fn probe(&self, root_pos: &Chess, winner: Color) -> io::Result<MoveList> {
        let mut pos = root_pos.clone();
        let mut move_list = MoveList::new();
        println!("FOO");
        loop {
            let moves = pos.legal_moves();
            println!("{:?}", pos.board());
            let (chess_move, best_outcome) = process_results(
                moves.iter().map(|chess_move| {
                    println!("{chess_move:?}");
                    let mut pos_after_move = pos.clone();
                    pos_after_move.play_unchecked(chess_move);
                    self.retrieve_outcome(&pos_after_move, winner)
                        .map(|outcome| dbg!((chess_move, outcome)))
                }),
                |iter| {
                    iter.max_by_key(|(_, outcome)| *outcome)
                        .expect("No outcomes found")
                },
            )?;
            move_list.push(chess_move.clone());
            pos.play_unchecked(chess_move);

            if best_outcome == Outcome::Win(0)
                || best_outcome == Outcome::Lose(0)
                || best_outcome == Outcome::Draw
            {
                break Ok(move_list);
            }
        }
    }
}

impl<T: Indexer> RetrieveOutcome for TablebaseProber<T> {
    fn raw_access_outcome(
        &self,
        mat: Material,
        pos: &Chess,
        winner: Color,
        flip: bool,
    ) -> std::io::Result<Outcome> {
        let lazy_file = self.0.get(&mat).expect("material config not included");
        dbg!(winner, flip, winner ^ flip);
        lazy_file.get(winner ^ flip).outcome_of(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use retroboard::shakmaty::{
        fen::Fen,
        CastlingMode, Chess,
        Color::{self, *},
        Position,
    };

    use paste::paste;
    use std::path::PathBuf;

    fn tb_test_dir() -> PathBuf {
        ["..", "table"].iter().collect()
    }

    fn check_pos_probe(fen: &str, outcome: Outcome, winner: Color) {
        let chess: Chess = Fen::from_ascii(fen.as_bytes())
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let tb_prober: TablebaseProber = TablebaseProber::new(&material, &tb_test_dir());
        assert_eq!(tb_prober.retrieve_outcome(&chess, winner).unwrap(), outcome);
    }

    // macro for generating tests
    macro_rules! gen_tests_probe {
    ($($fn_name:ident, $fen:tt, $outcome:expr, $winner:tt,)+) => {
        $(
        paste! {
            #[test]
            fn [<tests_probe_ $fn_name>]() {
                check_pos_probe($fen, $outcome, $winner);
            }
        }
        )+
    }
}

    gen_tests_probe! {
        without_switching_color_white, "1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1", Outcome::Win(1), White,
        with_switching_color_white, "3K4/1r2Q3/8/8/8/8/8/3k4 b - - 0 1", Outcome::Draw, White,
        without_switching_color_black, "1Qk5/6Q1/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, Black,
        with_switching_color_black,"8/8/8/8/8/1k6/3r4/1K1Q4 b - - 0 1",Outcome::Win(1), Black,
        qkvk_white_winner, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Win(10), White,
        qkvk_black_winner, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, Black,
    }
}
