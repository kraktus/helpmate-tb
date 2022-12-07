pub use helpmate_tb::{Common, EncoderDecoder, Material, MaterialWinner, TableBaseBuilder};

use log::info;

use retroboard::shakmaty::Color;
use std::str::FromStr;
use std::{fs::File, path::PathBuf};

use clap::{ArgAction, Args};

use crate::explore::stats;

#[derive(Args, Debug)]
pub struct Generate {
    #[arg(short, long, value_parser = Material::from_str, help = "example \"KQvK\"")]
    material: Material,
    #[arg(short, long, action = ArgAction::SetTrue)]
    recursive: bool,
    #[arg(long, default_value = if cfg!(feature = "syzygy") {"syzygy_table/"} else {"table/"})]
    tb_dir: PathBuf,
}

impl Generate {
    pub fn run(self) {
        let mut materials = if self.recursive {
            self.material.descendants_recursive(false)
        } else {
            vec![]
        };
        materials.push(self.material.clone());
        for mat in materials {
            self.gen_one_material(mat)
        }
    }

    fn gen_one_material(&self, mat: Material) {
        for winner in Color::ALL {
            info!("Building {mat:?} with winner: {winner}");
            // white first, most interesting
            let common = TableBaseBuilder::build(mat.clone(), winner, &self.tb_dir);
            let mat_win = MaterialWinner::new(&common.material, common.winner);
            let mut encoder = EncoderDecoder::new(
                File::create(self.tb_dir.join(format!("{mat_win:?}"))).unwrap(),
            );
            encoder
                .compress(&common.all_pos)
                .expect("Compression failed for mat {mat:?}");
            stats(mat_win, None, &common.all_pos, None)
        }
    }
}
