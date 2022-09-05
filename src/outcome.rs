use retroboard::shakmaty::ByColor;
use std::cmp::Ordering;
use std::ops::Add;
use std::ops::Not;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutcomeOutOfBound;

pub type Outcomes = Vec<ByColor<OutcomeU8>>;
pub type OutcomesSlice<'a> = &'a [ByColor<OutcomeU8>];

pub type Reports = Vec<ByColor<ReportU8>>;
pub type ReportsSlice<'a> = &'a [ByColor<ReportU8>];

/// Wrapper around `Outcome` to track if it has already been processed (ie retro moves generated) or not
/// When a position is generated it's `Unprocessed` by default.
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Report {
    Unprocessed(Outcome),
    Processed(Outcome),
}

#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub struct ReportU8(u8);

impl ReportU8 {
    pub fn from_raw_u8(u: u8) -> Self {
        Self(u)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub struct OutcomeU8(u8);

impl OutcomeU8 {
    pub fn from_raw_u8(u: u8) -> Option<Self> {
        if u < 128 {
            Some(Self(u))
        } else {
            None
        }
    }

    pub fn as_raw_u8(&self) -> u8 {
        self.0
    }
}

impl Report {
    #[inline]
    pub fn outcome(&self) -> Outcome {
        match self {
            Self::Unprocessed(outcome) => *outcome,
            Self::Processed(outcome) => *outcome,
        }
    }
}

impl From<Report> for ReportU8 {
    fn from(r: Report) -> Self {
        match r {
            Report::Unprocessed(outcome) => ReportU8(OutcomeU8::from(outcome).as_raw_u8()),
            Report::Processed(outcome) => ReportU8(OutcomeU8::from(outcome).as_raw_u8() + 128),
        }
    }
}

impl From<ReportU8> for Report {
    fn from(r: ReportU8) -> Self {
        if r.0 > 127 {
            Self::Processed(OutcomeU8(r.0 - 128).into())
        } else {
            Self::Unprocessed(OutcomeU8(r.0).into())
        }
    }
}

impl From<&ReportU8> for Report {
    fn from(u: &ReportU8) -> Self {
        (*u).into()
    }
}

/// According to winnner set in `Generator`. This struct need to fit in a u7
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
pub enum Outcome {
    // TODO replace by an enum with 63 elements?
    Win(u8), // Need to be between 0 and 63 excluded due to conversion to u7
    Unknown, // Used for positions we don't know the outcome yet. Cannot use `Draw` by default for positions where Drawing is the desired state (eg: KQvKb)
    Draw,
    // TODO replace by an enum with 63 elements?
    Lose(u8),  // Need to be between 0 and **62** excluded due to conversion to u7
    Undefined, // Used for illegal positions. Should we use Option<Outcome> without that variant instead?
}

pub const UNDEFINED_OUTCOME_BYCOLOR: ByColor<ReportU8> = ByColor {
    // Report::Processed(Outcome::Undefined).into()
    black: ReportU8(255),
    white: ReportU8(255),
};

impl From<OutcomeU8> for Outcome {
    fn from(u: OutcomeU8) -> Self {
        match u.0 {
            0 => Self::Draw,
            1 => Self::Unknown,
            127 => Self::Undefined,
            w if w > 63 => Self::Win(w - 64),
            l => Self::Lose(l - 2),
        }
    }
}

impl From<&OutcomeU8> for Outcome {
    fn from(u: &OutcomeU8) -> Self {
        (*u).into()
    }
}

impl Ord for Outcome {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Undefined, _) | (_, Self::Undefined) => {
                panic!("No Undefined/Unknown in comparison")
            }
            (Self::Win(x), Self::Win(y)) => x.cmp(y).reverse(), // short win is better,
            (Self::Win(_), Self::Draw | Self::Lose(_)) => Ordering::Greater, // if other is not a Win, we're greater
            (Self::Draw, Self::Win(_)) => Ordering::Less,
            (Self::Draw, Self::Draw) => Ordering::Equal,
            (Self::Draw, Self::Lose(_)) => Ordering::Greater,
            (Self::Lose(x), Self::Lose(y)) => x.cmp(y), // losing in many moves is better,
            (Self::Lose(_), Self::Win(_) | Self::Draw) => Ordering::Less,
            (Self::Unknown, _) => Ordering::Less,
            (_, Self::Unknown) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Outcome {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn try_into_util(o: Outcome) -> Result<OutcomeU8, OutcomeOutOfBound> {
    match o {
        Outcome::Draw => Ok(0),
        Outcome::Unknown => Ok(1),
        Outcome::Undefined => Ok(127),
        Outcome::Win(w) if w < 63 => Ok(w + 64),
        Outcome::Lose(l) if l < 62 => Ok(l + 2),
        _ => Err(OutcomeOutOfBound),
    }
    .map(|u| OutcomeU8::from_raw_u8(u).expect("Value is crafted such that it fits in u7"))
}

impl From<Outcome> for OutcomeU8 {
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
            Self::Undefined | Self::Unknown => panic!("Cannot invert undefined/unkown outcome"),
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
            Self::Undefined | Self::Unknown => panic!("Cannot add undefined/unkown outcome"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_outcome_to_u7() {
        assert_eq!(OutcomeU8::from(Outcome::Draw), OutcomeU8(0));
        assert_eq!(OutcomeU8::from(Outcome::Undefined), OutcomeU8(127));
        assert_eq!(OutcomeU8::from(Outcome::Lose(0)), OutcomeU8(2));
        assert_eq!(OutcomeU8::from(Outcome::Win(0)), OutcomeU8(64));
        assert_eq!(OutcomeU8::from(Outcome::Lose(61)), OutcomeU8(63));
    }

    #[test]
    fn test_u7_to_outcome() {
        for i in 0..127 {
            let outcome_u8 = OutcomeU8(i);
            assert_eq!(OutcomeU8::from(Outcome::from(outcome_u8)), outcome_u8)
        }
    }

    #[test]
    fn test_report_to_u8() {
        for outcome in [
            Outcome::Win(10),
            Outcome::Draw,
            Outcome::Lose(61),
            Outcome::Win(62),
            Outcome::Undefined,
            Outcome::Unknown,
        ] {
            println!("{:?}", outcome);
            assert_eq!(
                Report::Unprocessed(outcome),
                ReportU8::from(Report::Unprocessed(outcome)).into()
            );
            assert_eq!(
                Report::Processed(outcome),
                ReportU8::from(Report::Processed(outcome)).into()
            );
        }
    }

    #[test]
    fn test_u8_to_report() {
        for i in 0..u8::MAX {
            let report_u8 = ReportU8(i);
            assert_eq!(ReportU8::from(Report::from(report_u8)), report_u8)
        }
    }

    #[test]
    fn test_undefined_outcome_bycolor() {
        assert_eq!(
            UNDEFINED_OUTCOME_BYCOLOR,
            ByColor {
                black: Report::Processed(Outcome::Undefined).into(),
                white: Report::Processed(Outcome::Undefined).into(),
            }
        );
    }

    #[test]
    fn test_ord_outcome() {
        assert!(Outcome::Win(1) > Outcome::Win(2));
        assert!(Outcome::Win(100) > Outcome::Draw);
        assert!(Outcome::Win(100) > Outcome::Lose(1));
        assert!(Outcome::Draw > Outcome::Lose(1));
        assert!(Outcome::Lose(2) > Outcome::Lose(1));
    }

    #[test]
    #[should_panic]
    fn test_ord_outcome_panic() {
        let _ = Outcome::Undefined > Outcome::Win(1);
    }
}
