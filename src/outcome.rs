use std::ops::Add;
use std::ops::Not;
use std::cmp::Ordering;
use shakmaty::ByColor;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutcomeOutOfBound;

pub type Outcomes = Vec<ByColor<u8>>;
pub type OutcomesSlice<'a> = &'a [ByColor<u8>];

/// According to winnner set in `Generator`
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    // TODO replace by u7
    Win(u8), // Need to be between 0 and 63 excluded due to conversion to u7
    Draw,
    // TODO replace by u7
    Lose(u8),  // Need to be between 0 and 63 excluded due to conversion to u7
    Undefined, // Should we use Option<Outcome> without that variant instead?
}

pub const UNDEFINED_OUTCOME_BYCOLOR: ByColor<u8> = ByColor {
    black: 127,
    white: 127,
};

impl From<u8> for Outcome {
    fn from(u: u8) -> Self {
        match u {
            0 => Self::Draw,
            127 => Self::Undefined,
            w if w > 63 => Self::Win(w - 63),
            l => Self::Lose(l - 1),
        }
    }
}

impl From<&u8> for Outcome {
    fn from(u: &u8) -> Self {
        (*u).into()
    }
}

impl Ord for Outcome {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Win(x), Self::Win(y)) => x.cmp(y).reverse(), // short win is better,
            (Self::Win(_), Self::Draw | Self::Lose(_)) => Ordering::Greater, // if other is not a Win, we're greater
            (Self::Draw, Self::Win(_)) => Ordering::Less,
            (Self::Draw, Self::Draw) => Ordering::Equal,
            (Self::Draw, Self::Lose(_)) => Ordering::Greater,
            (Self::Lose(x), Self::Lose(y)) => x.cmp(y), // losing in many moves is better,
            (Self::Lose(_), Self::Win(_) | Self::Draw) => Ordering::Less,
            (Self::Undefined, _) | (_, Self::Undefined) => panic!("No Undefined in comparison"),
        }
    }
}

impl PartialOrd for Outcome {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn try_into_util(o: Outcome) -> Result<u8, OutcomeOutOfBound> {
    match o {
        Outcome::Draw => Ok(0),
        Outcome::Undefined => Ok(127),
        Outcome::Win(w) if w <= 63 => Ok(w + 63),
        Outcome::Lose(l) if l <= 63 => Ok(l + 1),
        _ => Err(OutcomeOutOfBound),
    }
}

impl From<Outcome> for u8 {
    fn from(o: Outcome) -> Self {
        try_into_util(o).unwrap()
    }
}

impl Not for Outcome {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Win(x) => Self::Lose(x),
            Self::Lose(x) => Self::Win(x),
            Self::Draw => Self::Draw,
            Self::Undefined => Self::Undefined,
        }
    }
}

impl Add<u8> for Outcome {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        match self {
            Self::Win(x) => Self::Win(x + rhs),
            Self::Lose(x) => Self::Lose(x + rhs),
            Self::Draw => Self::Draw,
            Self::Undefined => Self::Undefined,
        }
    }
}