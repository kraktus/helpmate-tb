use std::{collections::HashMap, path::Path};

use itertools::process_results;
use positioned_io::{RandomAccessFile, ReadAt};
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
        let raf = RandomAccessFile::open(tablebase_dir.join(format!("{mat:?}"))).unwrap();
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
        loop {
            let moves = pos.legal_moves();
            let (chess_move, best_outcome) = process_results(
                moves.iter().map(|chess_move| {
                    let mut pos_after_move = pos.clone();
                    pos_after_move.play_unchecked(chess_move);
                    self.retrieve_outcome(&pos_after_move, winner)
                        .map(|outcome| (chess_move, outcome))
                }),
                |iter| {
                    iter.max_by_key(|(_, outcome)| *outcome)
                        .expect("No outcomes found")
                },
            )?;

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
        lazy_file.get(winner ^ flip).outcome_of(pos)
    }
}
