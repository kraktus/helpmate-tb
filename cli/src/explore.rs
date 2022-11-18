pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};
use helpmate_tb::{DeIndexer, DefaultIndexer, FileHandler, IndexWithTurn};
use log::{debug, info};
use std::{collections::HashMap, path::{Path, PathBuf}, str::FromStr};

use retroboard::shakmaty::{ByColor, Color};

use clap::{ArgAction, Args};

#[derive(Debug, Clone)]
enum MatOrAll {
    Mat(Material),
    All,
}

impl FromStr for MatOrAll {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all" {
            Ok(Self::All)
        } else {
            Material::from_str(s).map(Self::Mat)
        }
    }
}

#[derive(Args, Debug)]
pub struct Explore {
    #[arg(help = "example \"KQvK\", use special value 'all' to search across all positions", value_parser = MatOrAll::from_str)]
    material: MatOrAll,
    #[arg(short, long, help = "Color of the expected winner", default_value_t = Color::White)]
    winner: Color,
    #[arg(long,
        value_parser = Outcome::from_str,
        help = "If draw is selected, only non-stalemate ones will be returned"
    )]
    outcome: Option<Outcome>,
    #[arg(long, action = ArgAction::SetFalse, default_value_t = false)]
    exclude_summary: bool,
    #[arg(short, long)]
    tb_dir: PathBuf,
}

impl Explore {
    pub fn run(&self) {
        match self.material {
            MatOrAll::All => {
                let entries = Path::new("./table")
                    .read_dir()
                    .expect("read_dir call failed");
                for entry_res in entries {
                    let mat_win_str = entry_res.unwrap().file_name().into_string().unwrap();
                    let mat_win =
                        MaterialWinner::from_str(&mat_win_str).expect("invalid file name");
                    self.stats_one_mat(mat_win);
                }
            }
            MatOrAll::Mat(ref mat) => {
                let mat_win = MaterialWinner::new(mat, self.winner);
                self.stats_one_mat(mat_win);
            }
        }
    }

    fn stats_one_mat(&self, mat_win: MaterialWinner) {
        info!(
            "Looking at {:?} with winner: {}",
            mat_win.material, mat_win.winner
        );
        let file_handler: FileHandler = FileHandler::new(&mat_win, &self.tb_dir);
        if !self.exclude_summary {
            stats(
                mat_win,
                Some(&file_handler.indexer),
                &file_handler.outcomes,
                self.outcome,
            )
        }
    }
}

pub fn stats<T>(
    mat_win: MaterialWinner,
    indexer: Option<&DefaultIndexer>,
    outcomes: &Vec<ByColor<T>>,
    searched_outcome: Option<Outcome>,
) where
    ByColor<T>: SideToMoveGetter,
{
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut unkown = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();
    let mut undefined_outcome: usize = 0;

    for (idx, by_color_outcome) in outcomes.iter().enumerate() {
        for turn in Color::ALL {
            let outcome = by_color_outcome.get_outcome_by_color(turn);
            if Some(outcome) == searched_outcome {
                info!(
                    "Macthing {outcome:?}, position {:?}",
                    indexer
                        .expect("Not indexer given depsite specific outcome being searched")
                        .restore(
                            &mat_win.material,
                            IndexWithTurn {
                                idx: idx as u64,
                                turn
                            }
                        )
                )
            }
            distrib.insert(outcome, *distrib.get(&outcome).unwrap_or(&0) + 1);
            match outcome {
                Outcome::Draw => draw += 1,
                Outcome::Win(_) => win += 1,
                Outcome::Lose(_) => lose += 1,
                Outcome::Undefined => undefined_outcome += 1,
                Outcome::Unknown => unkown += 1,
            }
        }
    }
    debug!(
        "From {:?} perspective, win: {win:?}, draw: {draw:?}, lost: {lose:?}, unkown: {unkown:?}",
        mat_win.winner
    );
    debug!(
        "Index density = {:?}%",
        (outcomes.len() * 2 - undefined_outcome) * 100 / (outcomes.len() * 2)
    );
    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Win(i)) {
            debug!("Win({}), {:?}", i, nb_win);
        }
    }

    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Lose(i)) {
            debug!("Lose({}), {:?}", i, nb_win);
        }
    }
}
