pub use helpmate_tb::{
    to_chess_with_illegal_checks, Common, EncoderDecoder, Material, MaterialWinner, TablebaseProber,
};
use helpmate_tb::{Outcome, RetrieveOutcome};

use log::info;

use retroboard::shakmaty::fen::Fen;
use retroboard::shakmaty::{Chess, Color, Position, PositionError};
use std::str::FromStr;
use std::{fs::File, path::PathBuf};

use clap::{ArgAction, Args};

use crate::explore::stats;

fn from_fen(fen: &str) -> Result<Chess, &'static str> {
    Fen::from_ascii(fen.as_bytes())
        .map_err(|_| "statically invalid fen")
        .and_then(|fen| {
            to_chess_with_illegal_checks(fen.into_setup()).map_err(|_| "illegal position")
        })
}

#[derive(Args, Debug)]
pub struct Probe {
    #[arg(short, long, value_parser = from_fen)]
    fen: Chess, // `fen` name to improve CLI usability. better would be to have the CLI still show `fen` but use `chess` internally
    #[arg(short, long)]
    winner: Color,
    #[arg(long)]
    tb_dir: PathBuf,
}

impl Probe {
    pub fn run(&self) {
        let material = Material::from_board(self.fen.board());
        let tb_prober: TablebaseProber = TablebaseProber::new(&material, &self.tb_dir);
        let outcome = tb_prober.retrieve_outcome(&self.fen, self.winner).unwrap();
        let mainline_len = match outcome {
            Outcome::Win(x) | Outcome::Lose(x) => x as usize,
            _ => 1,
        };
        // calling `probe` by construction ensures the line is legal
        assert_eq!(
            tb_prober.probe(&self.fen, self.winner).unwrap().len(),
            mainline_len
        );
    }
}
