use std::collections::HashMap;
use std::fmt;

use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{Chess, Color, Position};

use crate::{EncoderDecoder, Material, Outcome, Outcomes, SideToMoveGetter, Table};

#[derive(Debug)]
struct FileHandler {
    pub table: Table,
    pub outcomes: Outcomes,
}

impl FileHandler {
    pub fn new(mat: &MaterialWinner) -> Self {
        let raf = RandomAccessFile::open(format!("table/{mat:?}"))
            .expect("table file to be generated and accessible");
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("File well formated and readable");
        let table = Table::new(&mat.material);
        Self { table, outcomes }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct MaterialWinner {
    pub material: Material,
    pub winner: Color,
}

impl MaterialWinner {
    pub fn new(material: Material, winner: Color) -> Self {
        Self { material, winner }
    }
}
impl fmt::Debug for MaterialWinner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}{}",
            self.material,
            match self.winner {
                Color::Black => 'b',
                Color::White => 'w',
            }
        )
    }
}

#[derive(Debug)]
pub struct Descendants(HashMap<MaterialWinner, FileHandler>);

impl Descendants {
    pub fn new(mat: &Material) -> Option<Self> {
        let hashmap: HashMap<MaterialWinner, FileHandler> = mat
            .descendants_not_draw()
            .flat_map(|m| {
                Color::ALL.into_iter().map(move |winner| {
                    let mat_winner = MaterialWinner::new(m.clone(), winner);
                    let file_handler = FileHandler::new(&mat_winner);
                    (mat_winner, file_handler)
                })
            })
            .collect();
        if hashmap.is_empty() {
            None
        } else {
            Some(Self(hashmap))
        }
    }

    /// Returns the distance to helpmate in the descendant table, or panics
    fn retrieve_outcome(&self, pos: &Chess, winner: Color) -> Outcome {
        let mat = Material::from_board(pos.board());
        let table_file = self
            .0
            .get(&MaterialWinner::new(mat, winner))
            .expect("Position to be among descendants");
        let idx = table_file.table.encode(pos);
        table_file.outcomes[idx].get_by_pos(pos)
    }

    /// For the given position, compute all moves that are either captures and/or promotion,
    /// and return the best result
    /// Example:
    /// "KPvRK" where the pawn can take and promote then mate in 4, or just promote and mate in 2, will return `Outcome::Win(2)`
    pub fn outcome_from_captures_promotion(&self, pos: &Chess, winner: Color) -> Option<Outcome> {
        // TODO test function
        let mut moves = pos.legal_moves();
        moves.retain(|m| m.is_capture() || m.is_promotion());
        println!("{:?}", moves);
        moves
            .iter()
            .map(|chess_move| {
                let mut pos_after_move = pos.clone();
                pos_after_move.play_unchecked(chess_move);
                self.retrieve_outcome(&pos_after_move, winner)
            })
            .max()
            .map(|o| o + 1) // we are one move further from the max
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retroboard::shakmaty::Color::{Black, White};

    #[test]
    fn test_material_winner() {
        for ((m, c), expected_file_name) in [
            ((Material::from_str("KQvK").unwrap(), White), "KQvKw"),
            ((Material::from_str("KBvKN").unwrap(), Black), "KBvKNb"),
        ] {
            let mat_winner = MaterialWinner::new(m, c);
            assert_eq!(format!("{mat_winner:?}"), expected_file_name)
        }
    }
}
