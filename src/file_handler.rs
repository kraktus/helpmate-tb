use std::collections::HashMap;

use positioned_io::RandomAccessFile;
use shakmaty::Chess;
use shakmaty::Position;

use crate::{EncoderDecoder, Material, Outcome, Outcomes, SideToMoveGetter, Table};

#[derive(Debug)]
struct FileHandler {
    pub table: Table,
    pub outcomes: Outcomes,
}

impl FileHandler {
    pub fn new(mat: &Material) -> Self {
        let raf = RandomAccessFile::open(format!("table/{:?}", mat))
            .expect("table file to be generated and accessible");
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("File well formated and readable");
        let table = Table::new(mat);
        Self { table, outcomes }
    }
}

#[derive(Debug)]
pub struct TableBase(HashMap<Material, FileHandler>);

impl TableBase {
    pub fn new(mat: &Material) -> Option<Self> {
        let hashmap: HashMap<Material, FileHandler> = mat
            .descendants_not_draw()
            .map(|m| {
                let file_handler = FileHandler::new(&m);
                (m, file_handler)
            })
            .collect();
        if hashmap.is_empty() {
            None
        } else {
            Some(Self(hashmap))
        }
    }

    /// Returns the distance to helpmate in the descendant table, or panics
    fn retrieve_outcome(&self, pos: &Chess) -> Outcome {
        let mat = Material::from_board(pos.board());
        let table_file = self.0.get(&mat).expect("Position to be among descendants");
        let idx = table_file.table.encode(pos);
        table_file.outcomes[idx].get_by_pos(pos).outcome()
    }

    /// For the given position, compute all moves that are either captures and/or promotion,
    /// and return the best result
    /// Example:
    /// "KPvRK" where the pawn can take and promote then mate in 4, or just promote and mate in 2, will return `Outcome::Win(2)`
    pub fn outcome_from_captures_promotion(&self, pos: &Chess) -> Option<Outcome> {
        // TODO test function
        let mut moves = pos.legal_moves();
        moves.retain(|m| m.is_capture() || m.is_promotion());
        println!("{:?}", moves);
        moves
            .iter()
            .map(|chess_move| {
                let mut pos_after_move = pos.clone();
                pos_after_move.play_unchecked(chess_move);
                self.retrieve_outcome(&pos_after_move)
            })
            .max()
            .map(|o| o + 1) // we are one move further from the max
    }
}
