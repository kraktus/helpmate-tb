use std::{collections::VecDeque, marker::PhantomData, ops::Deref};

use retroboard::shakmaty::{ByColor, Color};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackedBools(u8);

impl PackedBools {
    const EMPTY: Self = Self(0);

    pub fn set_true(&mut self, idx: u8) {
        let bin_pattern = match idx {
            0 => 0b00000001u8,
            1 => 0b00000010u8,
            2 => 0b00000100u8,
            3 => 0b00001000u8,
            4 => 0b00010000u8,
            5 => 0b00100000u8,
            6 => 0b01000000u8,
            7 => 0b10000000u8,
            _ => unreachable!("idx should be between 0 and 7 included, was {idx}"),
        };
        self.0 |= bin_pattern;
    }
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
#[derive(Debug)]
struct MateInQueue<T> {
    inner: Vec<ByColor<PackedBools>>,
    inner_index: usize, // where we're in the OneQueue
    phantom: PhantomData<T>,
}

trait MateInN {}
trait MateInNPlus1 {}

impl<T> MateInQueue<T> {
    pub fn new(len: usize) -> Self {
        let inner: Vec<ByColor<PackedBools>> = vec![EMPTY_PACKED_BOOLS_BYCOLOR; len];
        Self {
            inner,
            inner_index: 0,
            phantom: PhantomData,
        }
    }
}

impl<T: MateInN> MateInQueue<T> {
    pub fn pop_front(&mut self) -> Option<IndexWithTurn> {
        while self.inner.get(self.inner_index)? == &EMPTY_PACKED_BOOLS_BYCOLOR {
            self.inner_index += 1;
        }
        // we know that at the current index
        for color in Color::ALL {
            let compressed_bools = self.inner[self.inner_index].get_mut(color);
            match compressed_bools.next() {
                Some(x) => {
                    return Some(IndexWithTurn {
                        idx: (self.inner_index * 8) as u64 + (x as u64),
                        turn: color,
                    })
                }
                None => (),
            }
        }

        None
    }
}

impl<T: MateInNPlus1> MateInQueue<T> {
    pub fn push_back(&mut self, idx_with_turn: IndexWithTurn) {
        let IndexWithTurn { idx, turn } = idx_with_turn;
        let inner_idx = (idx / 8) as usize;
        let compressed_bool_idx = idx % 8;
        self.inner[inner_idx]
            .get_mut(turn)
            .set_true(compressed_bool_idx as u8);
    }
}

#[cfg(test)]
mod tests {
    use retroboard::shakmaty::Color;

    use super::*;

    #[derive(Debug)]
    struct TestMarker;
    impl MateInN for TestMarker {}
    impl MateInNPlus1 for TestMarker {}

    #[test]
    fn test_packed_bool_iter() {
        let mut x = PackedBools(0b11111111u8);
        assert_eq!([0, 1, 2, 3, 4, 5, 6, 7], [0; 8].map(|_| x.next().unwrap()));
        assert_eq!(x, PackedBools::EMPTY);
    }

    #[test]
    fn test_packed_bool_set_true() {
        for i in 0..7 {
            let mut x = PackedBools::EMPTY;
            x.set_true(i);
            assert_eq!([i], [0; 1].map(|_| x.next().unwrap()))
        }
    }

    #[test]
    fn test_mate_in_x_push_and_pop() {
        let test_idx = [11278, 8945, 12, 3, 145]; // random
        for mul in test_idx {
            for div_rest in 0..7 {
                for turn in Color::ALL {
                    let mut mate_in_x: MateInQueue<TestMarker> =
                        MateInQueue::new(test_idx.into_iter().max().unwrap());
                    let idx = (mul + div_rest) as u64;
                    let idx_with_turn = IndexWithTurn { idx, turn };
                    mate_in_x.push_back(idx_with_turn);
                    println!("{:?}", &mate_in_x.inner[11278 / 8..11278 / 8 + 7]);
                    assert_eq!(idx_with_turn, mate_in_x.pop_front().unwrap());
                    // calling pop_front should remove the value from the queue
                    assert!(mate_in_x.pop_front().is_none());
                }
            }
        }
    }
}
