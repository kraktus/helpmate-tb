use std::collections::HashMap;
use std::fmt;

use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{ByColor, Chess, Color, Position};

use crate::{EncoderDecoder, Material, Outcome, Outcomes, SideToMoveGetter, Table};

#[derive(Debug)]
struct FileHandler {
    pub table: Table,
    pub outcomes: Outcomes,
}

impl FileHandler {
    pub fn new(mat: &'_ MaterialWinner) -> Self {
        let raf = RandomAccessFile::open(format!("table/{mat:?}"))
            .unwrap_or_else(|_| panic!("table not found {mat:?}"));
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("decompression failed");
        let table = Table::new(&mat.material);
        Self { table, outcomes }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct MaterialWinner<'a> {
    pub material: &'a Material,
    pub winner: Color,
}

impl<'a> MaterialWinner<'a> {
    pub fn new(material: &'a Material, winner: Color) -> Self {
        Self { material, winner }
    }
}
impl fmt::Debug for MaterialWinner<'_> {
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
pub struct Descendants(HashMap<Material, ByColor<FileHandler>>);

impl Descendants {
    pub fn new(mat: &Material) -> Option<Self> {
        let hashmap: HashMap<Material, ByColor<FileHandler>> = mat
            .descendants_not_draw()
            .map(|m| {
                (
                    m.clone(),
                    ByColor::new_with(|winner| {
                        let mat_winner = MaterialWinner::new(&m, winner);
                        FileHandler::new(&mat_winner)
                    }),
                )
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
            .get(&mat)
            .expect("Position to be among descendants")
            .get(winner);
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
            let mat_winner = MaterialWinner::new(&m, c);
            assert_eq!(format!("{mat_winner:?}"), expected_file_name)
        }
    }

    // TODO restore when positions where the desired outcome is drawing is well handled
    // #[test]
    // fn test_outcome_from_captures_promotion_without_switching_color() {
    //     let chess: Chess = Fen::from_ascii("1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1".as_bytes())
    //         .unwrap()
    //         .into_position(Standard)
    //         .unwrap();
    //     let material = Material::from_board(chess.board());
    //     let winner = White;
    //     let descendants = Descendants::new(&material).expect("KQvK descendant of KQvKR");
    //     assert_eq!(
    //         descendants.outcome_from_captures_promotion(&chess, winner),
    //         Some(Outcome::Win(1))
    //     );
    // }
}
