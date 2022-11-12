pub use helpmate_tb::{
    Common, EncoderDecoder, Material, MaterialWinner, Outcome, SideToMoveGetter, TableBaseBuilder,
    UNDEFINED_OUTCOME_BYCOLOR,
};

use log::info;

use retroboard::shakmaty::Color;
use std::fs::File;
use std::{collections::HashMap, str::FromStr};

use log::debug;

use clap::{ArgAction, Args};

#[derive(Args, Debug)]
pub struct Generate {
    #[arg(short, long, value_parser = Material::from_str, help = "example \"KQvK\"")]
    material: Material,
    #[arg(short, long, action = ArgAction::SetTrue)]
    recursive: bool,
}

impl Generate {
    pub fn run(self) {
        let mut materials = if self.recursive {
            self.material.descendants_recursive(false)
        } else {
            vec![]
        };
        materials.push(self.material);
        for mat in materials {
            gen_one_material(mat)
        }
    }
}

fn gen_one_material(mat: Material) {
    for winner in Color::ALL {
        info!("Generating {mat:?} with winner: {winner}");
        // white first, most interesting
        let common = TableBaseBuilder::build(mat.clone(), winner);
        let mut encoder = EncoderDecoder::new(
            File::create(format!(
                "../table/{:?}",
                MaterialWinner::new(&common.material, common.winner)
            ))
            .unwrap(),
        );
        encoder
            .compress(&common.all_pos)
            .expect("Compression failed for mat {mat:?}");
        stats(&common)
    }
}

fn stats(common: &Common) {
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut unkown = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();
    let mut undefined_outcome: usize = 0;

    for by_color_outcome in common.all_pos.iter() {
        if &UNDEFINED_OUTCOME_BYCOLOR == by_color_outcome {
            undefined_outcome += 2;
            continue;
        };
        for color in Color::ALL {
            let outcome = by_color_outcome.get_by_color(color).outcome();
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
        common.winner
    );
    debug!(
        "Index density = {:?}%",
        (common.all_pos.len() * 2 - undefined_outcome) * 100 / (common.all_pos.len() * 2)
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
