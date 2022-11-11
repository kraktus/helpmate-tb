#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::cast_possible_truncation,
    clippy::semicolon_if_nothing_returned // should be machine applicable
)]

mod common;
mod compression;
mod encoding;
mod file_handler;
mod generation;
mod indexer;
mod indexer_syzygy;
mod material;
mod outcome;

pub use crate::common::Common;
pub use crate::file_handler::{Descendants, FileHandler, MaterialWinner};
pub use crate::indexer::{DeIndexer, Indexer};
pub use crate::outcome::{
    Outcome, OutcomeU8, Outcomes, OutcomesSlice, Report, ReportU8, Reports, ReportsSlice,
    UNDEFINED_OUTCOME_BYCOLOR,
};
pub use compression::EncoderDecoder;
pub use encoding::get_info_table;
pub use generation::{
    to_chess_with_illegal_checks, Generator, IndexWithTurn, PosHandler, Queue, SideToMove,
    SideToMoveGetter, TableBaseBuilder,
};
use indexer::NaiveIndexer;
pub use indexer_syzygy::{Pieces, Table, A1_H8_DIAG, A8_H1_DIAG};
pub use material::{is_black_stronger, Material, KB_K, KN_K};

// type alias to allow to change all default type indexer from one position
pub type DefaultIndexer = NaiveIndexer;
