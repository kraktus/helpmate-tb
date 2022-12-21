use std::fmt;
use std::path::Path;
use std::{collections::HashMap, str::FromStr};

use log::trace;
use positioned_io::RandomAccessFile;
use retroboard::shakmaty::{ByColor, Chess, Color, Position};
use rustc_hash::FxHashMap;

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
        let table_path = tablebase_dir.join(format!("{mat:?}"));
        trace!("Creating new FileHandler for {table_path:?}");
        let raf = RandomAccessFile::open(&table_path).unwrap_or_else(|e| panic!("{e}, Most probably {table_path:?} not found, use `--recursive` option to regenerate descendants"));
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
pub struct Descendants<T = DefaultIndexer>(FxHashMap<MaterialWinner, FileHandler<T>>);

impl<T: Indexer + From<Material>> Descendants<T> {
    #[must_use]
    pub fn new(mat: &MaterialWinner, tablebase_dir: &Path) -> Self {
        let MaterialWinner { material, winner } = mat;
        let winners: Vec<Color> = if material.can_need_opposite_winner() {
            Color::ALL.into()
        } else {
            vec![*winner]
        };
        let mut hash_map: FxHashMap<MaterialWinner, FileHandler<T>> = FxHashMap::default();
        for m in material.descendants_not_draw() {
            for w in winners.iter() {
                let mat_win = MaterialWinner::new(&m, *w);
                hash_map.insert(mat_win.clone(), FileHandler::new(&mat_win, tablebase_dir));
            }
        }

        Self(hash_map)
    }

    // For test purpose
    #[must_use]
    pub fn empty() -> Self {
        Self(FxHashMap::default())
    }

    /// For the given position, compute all moves that are either captures and/or promotion,
    /// and return the best result
    /// Example:
    /// "`KPvRK`" where the pawn can take and promote then mate in 4, or just promote and mate in 2, will return `Outcome::Win(2)`
    /// Also return a boolean whose value is `true` if and only if all legal moves are promotion/captures
    #[must_use]
    pub fn outcome_from_captures_promotion(
        &self,
        pos: &Chess,
        winner: Color,
    ) -> Option<(Outcome, bool)> {
        let mut moves = pos.legal_moves();
        let all_moves_nb = moves.len();
        moves.retain(|m| m.is_capture() || m.is_promotion());
        let are_all_moves_captures = all_moves_nb == moves.len();
        moves
            .iter()
            .map(|chess_move| {
                let mut pos_after_move = pos.clone();
                pos_after_move.play_unchecked(chess_move);
                self.retrieve_outcome(&pos_after_move, winner)
                    .expect("No IO operation involved here")
            })
            .max()
            .map(|o| (o + 1, are_all_moves_captures)) // we are one move further from the max
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
        let mat_win = MaterialWinner::new(&mat, winner ^ flip);
        let table_file = self
            .0
            .get(&mat_win)
            .expect("Position to be among descendants");
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

    fn check_pos(fen: &str, outcome: Outcome, desired_are_all_moves_capture: bool, winner: Color) {
        let chess: Chess = Fen::from_ascii(fen.as_bytes())
            .unwrap()
            .into_position(Standard)
            .unwrap();
        let mat_win = MaterialWinner::new(&Material::from_board(chess.board()), winner);
        let descendants: Descendants = Descendants::new(&mat_win, &tb_test_dir());
        let (fetched_outcome, are_all_moves_capture) = descendants
            .outcome_from_captures_promotion(&chess, winner)
            .unwrap();
        assert_eq!(fetched_outcome, outcome);
        assert_eq!(desired_are_all_moves_capture, are_all_moves_capture);
    }

    // macro for generating tests
    macro_rules! gen_tests_descendants {
    ($($fn_name:ident, $fen:tt, $outcome:expr, $all_moves_capture:expr, $winner:tt,)+) => {
        $(
            paste! {
            #[test]
            fn [<tests_descendants_ $fn_name>]() {
                check_pos($fen, $outcome, $all_moves_capture, $winner);
            }
        }
        )+
    }
}

    // the tests are tested against files generated with the naive indexer
    gen_tests_descendants! {
        from_captures_promotion_without_switching_color_white, "1k6/1r6/1K6/8/4Q3/8/8/8 w - - 0 1", Outcome::Win(1), false, White,
        from_captures_promotion_with_switching_color_white, "3K4/1r2Q3/8/8/8/8/8/3k4 b - - 0 1", Outcome::Draw, false, White,
        promotion_without_switching_color_black, "1Qk5/6Q1/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, true, Black,
        promotion_with_switching_color_black,"8/8/8/8/8/1k6/3r4/1K1Q4 b - - 0 1",Outcome::Win(1), false, Black,
        special_case_only_2_kings_left_w, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, false, White,
        special_case_only_2_kings_left_b, "4k3/3Q4/8/8/8/8/8/3K4 b - - 0 1", Outcome::Draw, false, Black,
        all_moves_are_capture_w, "8/8/8/8/8/8/8/1KNkB3 b - -", Outcome::Draw, true, White,
        all_moves_are_capture_b, "8/8/8/8/8/8/8/1KNkB3 b - -", Outcome::Draw, true, Black,
    }
}
