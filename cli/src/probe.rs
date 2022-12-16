pub use helpmate_tb::{
    to_chess_with_illegal_checks, Common, EncoderDecoder, Material, MaterialWinner, TablebaseProber,
};
use helpmate_tb::{Indexer, NaiveIndexer, RetrieveOutcome};

use log::{debug, info};
use retroboard::shakmaty::fen::Fen;

use retroboard::shakmaty::{Chess, Color, Position};
use retroboard::RetroBoard;

use std::path::PathBuf;

use clap::{ArgAction, Args};

fn from_fen(fen: &str) -> Result<Chess, &'static str> {
    Fen::from_ascii(fen.as_bytes())
        .map_err(|_| "statically invalid fen")
        .and_then(|fen| {
            to_chess_with_illegal_checks(fen.into_setup()).map_err(|_| "illegal position")
        })
}

#[derive(Args, Debug)]
pub struct Probe {
    #[arg(short, long, value_parser = from_fen, name = "fen")]
    chess: Chess,
    #[arg(short, long)]
    winner: Color,
    #[arg(long, default_value = if cfg!(feature = "syzygy") {"syzygy_table/"} else {"table/"})]
    tb_dir: PathBuf,
    #[arg(long, action = ArgAction::SetTrue)]
    expanded: bool,
}

impl Probe {
    pub fn run(self) {
        let material = Material::from_board(self.chess.board());
        let tb_prober: TablebaseProber = TablebaseProber::new(&material, &self.tb_dir);
        let outcome = tb_prober
            .retrieve_outcome(&self.chess, self.winner)
            .unwrap();
        let (move_list, pos_list) = tb_prober.probe(&self.chess, self.winner).unwrap();
        let uci_movelist: Vec<String> = move_list
            .into_iter()
            .map(|m| {
                m.to_uci(retroboard::shakmaty::CastlingMode::Standard)
                    .to_string()
            })
            .collect();
        let rboard = RetroBoard::from(self.chess);
        info!(
            "For {:?}\nOutcome is {outcome:?}, Moves: {uci_movelist:?}",
            rboard,
        );
        debug!("Naive indexer idx: {:?}", NaiveIndexer.encode(&rboard));
        if self.expanded {
            let rboards_fmt: Vec<String> = pos_list
                .into_iter()
                .map(|p| {
                    let r = RetroBoard::from(p);
                    let idx = NaiveIndexer.encode(&r);
                    format!("{r:?}, idx: {idx:?}")
                })
                .collect();
            info!("{}", rboards_fmt.join("\n"));
        }
    }
}
