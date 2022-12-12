use std::{mem, ops::Deref};

use retroboard::shakmaty::{ByColor, Color};

use crate::{DeIndexer, DefaultReversibleIndexer, IndexWithTurn};

// TODO this whole file should be able to be deleted by working directly on the all_idx vec
// Because the naive inverser is reversible
#[derive(Debug, Clone, Default)]
pub struct Queue<T = DefaultReversibleIndexer> {
    // depending on the material configuration can be either won or drawn position
    pub desired_outcome_pos_to_process: Vec<IndexWithTurn>,
    pub losing_pos_to_process: Vec<IndexWithTurn>,
    reversible_indexer: T,
}

/// Wrapper containing the informations needed to process "one queue", ie initialise from a vec of desired outcome
/// Then allow to store the positions that will need to be check in the next pass of the `Tagger` more efficiently than a traditional
///  VecDeque, using packed bools.
#[derive(Debug, Clone)]
pub struct OneQueue {
    pub mate_in_n: MateInQueue,
    pub mate_in_n_plus_1: MateInQueue,
    // nb_pass: usize, // number of `Tagger` pass done
}

impl OneQueue {
    pub fn new(desired_outcome_pos_to_process: Vec<IndexWithTurn>, all_pos_idx_len: usize) -> Self {
        let mut mate_in_n = MateInQueue::new(all_pos_idx_len / 8 + 1);
        for idx_with_turn in desired_outcome_pos_to_process.into_iter() {
            mate_in_n.push_back(idx_with_turn);
        }
        let mate_in_n_plus_1 = MateInQueue::new(all_pos_idx_len / 8 + 1);
        Self {
            mate_in_n,
            mate_in_n_plus_1,
        }
    }

    pub fn new_empty(all_pos_idx_len: usize) -> Self {
        let mate_in_n = MateInQueue::new(all_pos_idx_len / 8 + 1);
        let mate_in_n_plus_1 = MateInQueue::new(all_pos_idx_len / 8 + 1);
        Self {
            mate_in_n,
            mate_in_n_plus_1,
        }
    }

    pub fn push_back(&mut self, idx_with_turn: IndexWithTurn) {
        self.mate_in_n_plus_1.push_back(idx_with_turn)
    }

    pub fn pop_front(&mut self) -> Option<IndexWithTurn> {
        self.mate_in_n.pop_front()
    }

    // pub fn nb_pass(&mut self) -> usize {
    //     self.nb_pass
    // }

    /// To be called at the end of a `Tagger` pass. `mate_in_n` must have been completely emptied
    /// So we must take the N+1 queue and move it in the place of the N queue
    pub fn swap(&mut self) {
        self.mate_in_n.reset_counter();
        // debug_assert!(self.mate_in_n.is_empty());
        // self.nb_pass += 1;
        mem::swap(&mut self.mate_in_n, &mut self.mate_in_n_plus_1)
    }
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

#[derive(Debug, Clone)]
pub struct MateInQueue {
    inner: Vec<ByColor<PackedBools>>,
    inner_index: usize, // where we're in the OneQueue
}

impl MateInQueue {
    pub fn new(len: usize) -> Self {
        let mut inner: Vec<ByColor<PackedBools>> = vec![EMPTY_PACKED_BOOLS_BYCOLOR; len];
        inner.shrink_to_fit();
        Self {
            inner,
            inner_index: 0,
        }
    }

    pub fn reset_counter(&mut self) {
        debug_assert!(self.inner_index == self.inner.len());
        self.inner_index = 0;
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        for i in 0..self.inner.len() {
            if self.inner[i] != EMPTY_PACKED_BOOLS_BYCOLOR {
                return false;
            }
        }
        true
    }

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
    use retroboard::shakmaty::Color::*;

    use super::*;

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
                    let mut mate_in_x = MateInQueue::new(test_idx.into_iter().max().unwrap());
                    let idx = (mul + div_rest) as u64;
                    let idx_with_turn = IndexWithTurn { idx, turn };
                    mate_in_x.push_back(idx_with_turn);
                    assert_eq!(idx_with_turn, mate_in_x.pop_front().unwrap());
                    // calling pop_front should remove the value from the queue
                    assert!(mate_in_x.pop_front().is_none());
                }
            }
        }
    }

    #[test]
    fn test_one_queue() {
        let test_idx = Vec::from([
            IndexWithTurn {
                idx: 11278,
                turn: White,
            },
            IndexWithTurn {
                idx: 8945,
                turn: Black,
            },
            IndexWithTurn {
                idx: 12,
                turn: Black,
            },
            IndexWithTurn {
                idx: 3,
                turn: White,
            },
            IndexWithTurn {
                idx: 145,
                turn: Black,
            },
            IndexWithTurn {
                idx: 568,
                turn: White,
            },
            IndexWithTurn {
                idx: 4812,
                turn: White,
            },
        ]); // random
            // created with the number len of index, but since it's packed
            // the effective len is divided by two
        let mut one_queue = OneQueue::new(test_idx, 12_000 * 8);
        for _ in 0..2 {
            while let Some(mut idx_with_turn) = one_queue.pop_front() {
                idx_with_turn.idx += 1;
                one_queue.push_back(idx_with_turn);
            }
            assert!(one_queue.mate_in_n.is_empty());
            // the N+1 queue should NOT be empty
            assert!(!one_queue.mate_in_n_plus_1.is_empty());
            one_queue.swap();
            // after swapping the N queue should NOT be empty
            assert!(!one_queue.mate_in_n.is_empty());
            assert!(one_queue.mate_in_n_plus_1.is_empty())
        }
        assert_eq!(
            [0; 7].map(|_| one_queue.mate_in_n.pop_front().unwrap()),
            [
                IndexWithTurn {
                    idx: 5,
                    turn: White,
                },
                IndexWithTurn {
                    idx: 14,
                    turn: Black,
                },
                IndexWithTurn {
                    idx: 147,
                    turn: Black,
                },
                IndexWithTurn {
                    idx: 570,
                    turn: White,
                },
                IndexWithTurn {
                    idx: 4814,
                    turn: White,
                },
                IndexWithTurn {
                    idx: 8947,
                    turn: Black,
                },
                IndexWithTurn {
                    idx: 11280,
                    turn: White,
                },
            ]
        )
    }
}
