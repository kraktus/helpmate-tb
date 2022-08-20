use std::collections::HashMap;
use std::{fmt, io};

use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{Chess, Color, Position};

use crate::{EncoderDecoder, Material, Outcome, Outcomes, SideToMoveGetter, Table};

#[derive(Debug)]
struct FileHandler {
    pub table: Table,
    pub outcomes: Outcomes,
}

impl FileHandler {
    pub fn new(mat: &MaterialWinner) -> io::Result<Self> {
        let raf = RandomAccessFile::open(format!("table/{mat:?}"))?;
        let outcomes = EncoderDecoder::new(raf).decompress_file()?;
        let table = Table::new(&mat.material);
        Ok(Self { table, outcomes })
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
                Color::ALL.into_iter().flat_map(move |winner| {
                    let mat_winner = MaterialWinner::new(m.clone(), winner);
                    FileHandler::new(&mat_winner).map(|file_handler| (mat_winner, file_handler))
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
    use retroboard::shakmaty::{
        fen::Fen,
        CastlingMode::Standard,
        Color::{Black, White},
    };

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

    #[test]
    fn test_outcome_from_captures_promotion_without_switching_color() {
        let chess: Chess = Fen::from_ascii("1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1".as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let winner = White;
        let descendants = Descendants::new(&material).expect("KQvK descendant of KQvKR");
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(Outcome::Win(1))
        );
    }
}
