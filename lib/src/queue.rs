use std::{collections::VecDeque, ops::Deref};

use retroboard::shakmaty::ByColor;

use crate::{DeIndexer, DefaultReversibleIndexer, IndexWithTurn};

// the index is independant of the turn, so must be stored separately
#[derive(Debug, Clone, Default)]
pub struct Queue<T = DefaultReversibleIndexer> {
    // depending on the material configuration can be either won or drawn position
    pub desired_outcome_pos_to_process: VecDeque<IndexWithTurn>,
    pub losing_pos_to_process: VecDeque<IndexWithTurn>,
    reversible_indexer: T,
}

impl<T: DeIndexer + DeIndexer> Deref for Queue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.reversible_indexer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackedBools(u8);

impl PackedBools {
    const EMPTY: Self = Self(0);
}

macro_rules! if_bin_pattern_yield {
    ($number:expr, $bit_pattern:literal, $yield_value:literal) => {
        if ($number & $bit_pattern != 0) {
            $number -= $bit_pattern;
            return Some($yield_value);
        }
    };
}

impl Iterator for PackedBools {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if_bin_pattern_yield!(self.0, 0b00000001u8, 0);
        if_bin_pattern_yield!(self.0, 0b00000010u8, 1);
        if_bin_pattern_yield!(self.0, 0b00000100u8, 2);
        if_bin_pattern_yield!(self.0, 0b00001000u8, 3);
        if_bin_pattern_yield!(self.0, 0b00010000u8, 4);
        if_bin_pattern_yield!(self.0, 0b00100000u8, 5);
        if_bin_pattern_yield!(self.0, 0b01000000u8, 6);
        if_bin_pattern_yield!(self.0, 0b10000000u8, 7);
        None
    }
}

const EMPTY_PACKED_BOOLS_BYCOLOR: ByColor<PackedBools> = ByColor {
    black: PackedBools::EMPTY,
    white: PackedBools::EMPTY,
};

/// T being a marker, N or N+1
struct MateInQueue<T> {
    inner: Vec<ByColor<PackedBools>>,
    inner_index: usize, // where we're in the OneQueue
    phantom: std::marker::PhantomData<T>,
}

trait MateInN {}
trait MateInNPlus1 {}

impl<T: MateInN> MateInQueue<T> {
    pub fn pop_front(&mut self) -> IndexWithTurn {
        while self.inner[self.inner_index] == EMPTY_PACKED_BOOLS_BYCOLOR {
            self.inner_index += 1;
        }
        // we know that at the current index
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_bool_iter() {
        assert_eq!(
            vec![0, 1, 2, 3, 4, 5, 6, 7],
            PackedBools(0b11111111u8).collect::<Vec<u8>>()
        )
    }
}
