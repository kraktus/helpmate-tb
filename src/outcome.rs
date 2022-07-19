use shakmaty::ByColor;
use std::cmp::Ordering;
use std::ops::Add;
use std::ops::Not;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutcomeOutOfBound;

pub type Outcomes = Vec<ByColor<u8>>;
pub type OutcomesSlice<'a> = &'a [ByColor<u8>];

/// Wrapper around `Outcome` to track if it has already been processed (ie retro moves generated) or not
/// When a position is generated it's `Unprocessed` by default.
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Report {
    Unprocessed(Outcome),
    Processed(Outcome),
}

// impl Report {
//     #[inline]
//     fn outcome(&self) -> &Outcome {
//         match self {
//             Self::Unprocessed(ref outcome) => outcome,
//             Self::Processed(ref outcome) => outcome,
//         }
//     }
// }

impl From<Report> for u8 {
    fn from(r: Report) -> Self {
        match r {
            Report::Unprocessed(outcome) => u8::from(outcome),
            Report::Processed(outcome) => u8::from(outcome) + 128,
        }
    }
}

impl From<u8> for Report {
    fn from(u: u8) -> Self {
        if u > 127 {
            Self::Processed((u - 128).into())
        } else {
            Self::Unprocessed(u.into())
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_outcome_to_u7() {
        assert_eq!(u8::try_from(Outcome::Draw).unwrap(), 0);
        assert_eq!(u8::try_from(Outcome::Undefined).unwrap(), 127);
        assert_eq!(u8::try_from(Outcome::Lose(0)).unwrap(), 1);
        assert_eq!(u8::try_from(Outcome::Lose(62)).unwrap(), 63);
    }

    #[test]
    fn test_u7_to_outcome() {
        for i in 0..127 {
            assert_eq!(u8::try_from(Outcome::from(i)).unwrap(), i)
        }
    }

    #[test]
    fn test_report_to_u8() {
        for outcome in [
            Outcome::Win(10),
            Outcome::Draw,
            Outcome::Lose(62),
            Outcome::Win(62),
            Outcome::Undefined,
        ] {
            println!("{:?}", outcome);
            assert_eq!(
                Report::Unprocessed(outcome),
                u8::from(Report::Unprocessed(outcome)).into()
            );
            assert_eq!(
                Report::Processed(outcome),
                u8::from(Report::Processed(outcome)).into()
            );
        }
    }
}
