use crate::{
    indexer::Indexer, DefaultIndexer, Material, MaterialWinner, Reports, UNDEFINED_OUTCOME_BYCOLOR,
};

use indicatif::{ProgressBar, ProgressStyle};
use log::trace;
use retroboard::shakmaty::Color;

#[derive(Debug)]
pub struct Common<T = DefaultIndexer> {
    pub all_pos: Reports,
    pub counter: u64,
    mat_win: MaterialWinner,
    can_mate: bool, // if `true`, the desired outcome is winning, otherwise it's to draw
    indexer: T,
}

impl<T: From<Material>> Common<T> {
    #[must_use]
    pub fn new(mat_win: MaterialWinner) -> Self {
        trace!("Creating a new `Common` instance");
        Self {
            all_pos: vec![UNDEFINED_OUTCOME_BYCOLOR; get_estimate_nb_pos(&mat_win.material)],
            counter: 0,
            can_mate: mat_win.material.can_mate(mat_win.winner),
            indexer: T::from(mat_win.material.clone()),
            mat_win,
        }
    }
}

impl<T> Common<T> {
    #[must_use]
    pub fn get_progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new((get_estimate_nb_pos(&self.material()) * 2) as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
            .expect("Invalid indicatif template syntax")
            .progress_chars("#>-"),
        );
        pb
    }

    #[must_use]
    pub fn can_mate(&self) -> bool {
        self.can_mate
    }

    #[must_use]
    pub fn material(&self) -> &Material {
        &self.mat_win.material
    }

    #[must_use]
    pub fn winner(&self) -> Color {
        self.mat_win.winner
    }

    #[must_use]
    pub fn material_winner(&self) -> &MaterialWinner {
        &self.mat_win
    }
}

impl<T: Indexer> Common<T> {
    #[must_use]
    pub fn indexer(&self) -> &T {
        &self.indexer
    }
}

#[inline]
fn get_estimate_nb_pos(mat: &Material) -> usize {
    // white king is already included in `material.count()`, so substract it, and multiply by 10 instead, real number of cases the white king can go on
    // heuristic, less than 92% of pos are legals.
    (pow_minus_1(63, mat.count() - 1) * 10) as usize / 100 * 92
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pow_minus_1() {
        assert_eq!(pow_minus_1(64, 1), 64);
        assert_eq!(pow_minus_1(64, 2), 64 * 63);
    }
}
