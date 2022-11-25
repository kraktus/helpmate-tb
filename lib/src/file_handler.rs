use std::fmt;
use std::path::Path;
use std::{collections::HashMap, str::FromStr};

use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{ByColor, Chess, Color, Position};

use crate::{
    indexer::Indexer, is_black_stronger, DefaultIndexer, EncoderDecoder, Material, Outcome,
    Outcomes, SideToMoveGetter, KB_K, KN_K,
};

#[derive(Debug)]
pub struct FileHandler<T = DefaultIndexer> {
    pub indexer: T, // needed in case we want to re-extract the position from the index if reversible
    pub outcomes: Outcomes,
}

impl<T: From<Material>> FileHandler<T> {
    #[must_use]
    pub fn new(mat: &MaterialWinner, tablebase_dir: &Path) -> Self {
        let raf = RandomAccessFile::open(tablebase_dir.join(format!("{mat:?}"))).unwrap();
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("decompression failed");
        let indexer = T::from(mat.material.clone());
        Self { indexer, outcomes }
    }
}

#[derive(Eq, Hash, PartialEq, Clone)]
pub struct MaterialWinner {
    pub material: Material,
    pub winner: Color,
}

impl MaterialWinner {
    #[must_use]
    pub fn new(material: &Material, winner: Color) -> Self {
        Self {
            material: material.clone(),
            winner,
        }
    }
}

impl FromStr for MaterialWinner {
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
        Ok(Self { material, winner })
    }
}

impl fmt::Debug for MaterialWinner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{}", self.material, self.winner.char())
    }
}

#[derive(Debug)]
pub struct Descendants<T = DefaultIndexer>(HashMap<Material, ByColor<FileHandler<T>>>);

impl<T: Indexer + From<Material>> Descendants<T> {
    #[must_use]
    pub fn new(mat: &Material, tablebase_dir: &Path) -> Self {
        Self(
            mat.descendants_not_draw()
                .map(|m| {
                    (
                        m.clone(),
                        ByColor::new_with(|winner| {
                            let mat_winner = MaterialWinner::new(&m, winner);
                            FileHandler::new(&mat_winner, tablebase_dir)
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
                    .expect("No IO operation involved here")
            })
            .max()
            .map(|o| o + 1) // we are one move further from the max
    }
}

pub trait RetrieveOutcome {
    fn raw_access_outcome(
        &self,
        mat: Material,
        pos: &Chess,
        winner: Color,
        flip: bool,
    ) -> std::io::Result<Outcome>;

    /// Returns the distance to helpmate in the descendant table, or panics
    fn retrieve_outcome(&self, pos: &Chess, winner: Color) -> std::io::Result<Outcome> {
        let flip = is_black_stronger(pos.board());
        let mat = Material::from_board(pos.board());
        // special case for material config known to be draw in every position
        if mat.count() == 2 || mat == KB_K || mat == KN_K {
            return Ok(Outcome::Draw);
        }
        self.raw_access_outcome(mat, pos, winner, flip)
    }
}

impl<T: Indexer> RetrieveOutcome for Descendants<T> {
    fn raw_access_outcome(
        &self,
        mat: Material,
        pos: &Chess,
        winner: Color,
        flip: bool,
    ) -> std::io::Result<Outcome> {
        let table_file = self
            .0
            .get(&mat)
            .expect("Position to be among descendants")
            .get(winner ^ flip);
        let idx = table_file.indexer.encode(pos).usize();
        Ok(table_file.outcomes[idx].get_by_color(pos.turn() ^ flip))
    }
}

#[cfg(test)]
mod tests {
    use paste::paste;

    use super::*;
    use retroboard::shakmaty::{
        fen::Fen,
        CastlingMode::Standard,
        Color::{Black, White},
    };

    use std::{path::PathBuf, str::FromStr};

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

    fn tb_test_dir() -> PathBuf {
        ["..", "table"].iter().collect()
    }

    fn check_pos(fen: &str, outcome: Outcome, winner: Color) {
        let chess: Chess = Fen::from_ascii(fen.as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let material = Material::from_board(chess.board());
        let descendants: Descendants = Descendants::new(&material, &tb_test_dir());
        assert_eq!(
            descendants.outcome_from_captures_promotion(&chess, winner),
            Some(outcome)
        );
    }

    // macro for generating tests
    macro_rules! gen_tests_descendants {
    ($($fn_name:ident, $fen:tt, $outcome:expr, $winner:tt,)+) => {
        $(
            paste! {
            #[test]
            fn [<tests_descendants_ $fn_name>]() {
                check_pos($fen, $outcome, $winner);
            }
        }
        )+
    }
}

    // should be kept in sync with `probe.rs` tests
    gen_tests_descendants! {
        from_captures_promotion_without_switching_color_white, "1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1", Outcome::Win(1), White,
        from_captures_promotion_with_switching_color_white, "3K4/1r2Q3/8/8/8/8/8/3k4 b - - 0 1", Outcome::Draw, White,
        promotion_without_switching_color_black, "1Qk5/6Q1/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, Black,
        promotion_with_switching_color_black,"8/8/8/8/8/1k6/3r4/1K1Q4 b - - 0 1",Outcome::Win(1), Black,
        special_case_only_2_kings_left_w, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, White,
        special_case_only_2_kings_left_b, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, Black,
    }
}
