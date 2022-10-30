use crate::{Material, Reports, Table, UNDEFINED_OUTCOME_BYCOLOR};

use indicatif::{ProgressBar, ProgressStyle};
use retroboard::shakmaty::Color;

#[derive(Debug)]
pub struct Common {
    pub all_pos: Reports,
    pub winner: Color,
    pub counter: u64,
    pub material: Material,
    can_mate: bool, // if `true`, the desired outcome is winning, otherwise it's to draw
    index_table: Table,
}

impl Common {
    pub fn new(material: Material, winner: Color) -> Self {
        Self {
            index_table: Table::new(&material),
            all_pos: vec![UNDEFINED_OUTCOME_BYCOLOR; get_nb_pos(&material) as usize / 10 * 9], // heuristic, less than 90% of pos are legals. Takes x2 (because each stored element is in fact 1 position, but with black and white to turn) more than number of legal positions
            winner,
            counter: 0,
            can_mate: material.can_mate(winner),
            material,
        }
    }

    pub fn get_progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(get_nb_pos(&self.material));
        pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));
        pb
    }

    pub fn can_mate(&self) -> bool {
        self.can_mate
    }

    pub fn index_table(&self) -> &Table {
        &self.index_table
    }
}

#[inline]
fn get_nb_pos(mat: &Material) -> u64 {
    // white king is already included in `material.count()`, so substract it, and multiply by 10 instead, real number of cases the white king can go on
    pow_minus_1(63, mat.count() - 1) * 10 * 2
}

// instead of 64**4 get 64*63*62*61
#[inline]
const fn pow_minus_1(exp: u64, left: usize) -> u64 {
    if left > 0 {
        exp * pow_minus_1(exp - 1, left - 1)
    } else {
        1
    }
}
