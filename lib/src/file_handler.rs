use std::borrow::Cow;
use std::fmt;
use std::{collections::HashMap, str::FromStr};

use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{ByColor, Chess, Color, Position};

use crate::{
    indexer::Indexer, is_black_stronger, DefaultIndexer, EncoderDecoder, Material, Outcome,
    Outcomes, SideToMoveGetter, KB_K, KN_K,
};

#[derive(Debug)]
pub struct FileHandler<T = DefaultIndexer> {
    pub indexer: T,
    pub outcomes: Outcomes,
}

impl<T: Indexer> FileHandler<T> {
    pub fn new(mat: &MaterialWinner) -> Self {
        let raf = RandomAccessFile::open(format!("../table/{mat:?}")).unwrap();
        // .unwrap_or_else(|e| {
        //     panic!("table not found {e:?}, run from the root directory of the project")
        // });
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("decompression failed");
        let indexer = T::new(&mat.material);
        Self { indexer, outcomes }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct MaterialWinner<'a> {
    pub material: Cow<'a, Material>,
    pub winner: Color,
}

impl<'a> MaterialWinner<'a> {
    #[must_use]
    pub fn new(material: &'a Material, winner: Color) -> Self {
        Self {
            material: Cow::Borrowed(material),
            winner,
        }
    }
}

impl FromStr for MaterialWinner<'_> {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err("material should only contain ascii cases");
        }
        let full_string = s.to_string();
        let (mat_str, color_str) = full_string.split_at(s.len() - 1);
        let winner = char::from_str(color_str)
            .ok()
            .and_then(Color::from_char)
            .ok_or("last char must be 'b' for black or 'w' for white")?;
        let material = Material::from_str(mat_str).expect("Valid material config");
        Ok(Self {
            material: Cow::Owned(material),
            winner,
        })
    }
}

impl fmt::Debug for MaterialWinner<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{}", self.material, self.winner.char())
    }
}

#[derive(Debug)]
pub struct Descendants<T = DefaultIndexer>(HashMap<Material, ByColor<FileHandler<T>>>);

impl<T: Indexer> Descendants<T> {
    #[must_use]
    pub fn new(mat: &Material) -> Self {
        Self(
            mat.descendants_not_draw()
                .map(|m| {
                    (
                        m.clone(),
                        ByColor::new_with(|winner| {
                            let mat_winner = MaterialWinner::new(&m, winner);
                            FileHandler::new(&mat_winner)
                        }),
                    )
                })
                .collect(),
        )
    }

    // For test purpose
    #[must_use]
    pub fn empty() -> Self {
        Self(HashMap::new())
    }

    /// Returns the distance to helpmate in the descendant table, or panics
    fn retrieve_outcome(&self, pos: &Chess, winner: Color) -> Outcome {
        let flip = is_black_stronger(pos.board());
        let mat = Material::from_board(pos.board());
        // special case for material config known to be draw in every position
        if mat.count() == 2 || mat == KB_K || mat == KN_K {
            return Outcome::Draw;
        }
        let table_file = self
            .0
            .get(&mat)
            .expect("Position to be among descendants")
            .get(winner ^ flip);
        let idx = table_file.indexer.encode(pos).usize();
        table_file.outcomes[idx].get_by_color(pos.turn() ^ flip)
    }

    /// For the given position, compute all moves that are either captures and/or promotion,
    /// and return the best result
    /// Example:
    /// "`KPvRK`" where the pawn can take and promote then mate in 4, or just promote and mate in 2, will return `Outcome::Win(2)`
    #[must_use]
    pub fn outcome_from_captures_promotion(&self, pos: &Chess, winner: Color) -> Option<Outcome> {
        let mut moves = pos.legal_moves();
        moves.retain(|m| m.is_capture() || m.is_promotion());
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

    use std::str::FromStr;

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

    #[cfg(not(miri))]
    #[test]
    fn test_outcome_from_captures_promotion_without_switching_color_white() {
        let chess: Chess = Fen::from_ascii("1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1".as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let winner = White;
        let descendants: Descendants = Descendants::new(&material);
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(Outcome::Win(1))
        );
    }

    #[cfg(not(miri))]
    #[test]
    fn test_outcome_from_captures_promotion_with_switching_color_white() {
        let chess: Chess = Fen::from_ascii("3K4/1r2Q3/8/8/8/8/8/3k4 b - - 0 1".as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let winner = White;
        let descendants: Descendants = Descendants::new(&material);
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(Outcome::Draw)
        );
    }

    #[cfg(not(miri))]
    #[test]
    fn test_outcome_from_captures_promotion_without_switching_color_black() {
        let chess: Chess = Fen::from_ascii("1Qk5/6Q1/8/8/8/8/8/3K4 b - - 0 1".as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let winner = Black;
        let descendants: Descendants = Descendants::new(&material);
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(Outcome::Draw)
        );
    }

    #[cfg(not(miri))]
    #[test]
    fn test_outcome_from_captures_promotion_with_switching_color_black() {
        let chess: Chess = Fen::from_ascii("8/8/8/8/8/1k6/3r4/1K1Q4 b - - 0 1".as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let winner = Black;
        let descendants: Descendants = Descendants::new(&material);
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(Outcome::Win(1))
        );
    }

    #[test]
    fn test_outcome_from_captures_special_case_only_2_kings_left() {
        for winner in Color::ALL {
            let chess: Chess = Fen::from_ascii("4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1".as_bytes())
                .unwrap()
                .into_position(Standard)
                .unwrap();
            let material = Material::from_board(chess.board());
            let descendants: Descendants = Descendants::new(&material);
            assert_eq!(
                descendants.outcome_from_captures_promotion(&chess, winner),
                Some(Outcome::Draw)
            );
        }
    }
}
