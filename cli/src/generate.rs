use from_str_sequential::FromStrSequential;
pub use helpmate_tb::{Common, EncoderDecoder, Material, MaterialWinner, TableBaseBuilder};

use log::info;

use retroboard::shakmaty::Color;
use std::{fs::File, path::PathBuf};

use clap::{ArgAction, Args};

use crate::check_indexer::MatOrNbPieces;
use crate::explore::stats;

#[derive(Args, Debug)]
pub struct Generate {
    #[arg(short, long, value_parser = MatOrNbPieces::from_str_sequential, help = "maximum number of pieces on the board, will build all pawnless material config up to this number included.\nOr just a particular material configuration. Note that if a number is given, not compatible with --recursive")]
    mat_or_nb_pieces: MatOrNbPieces,
    #[arg(short, long, action = ArgAction::SetTrue)]
    recursive: bool,
    #[arg(long, default_value = "table/")]
    tb_dir: PathBuf,
    #[arg(
        short,
        long,
        help = "Color of the expected winner. If no color is provided, will search for both"
    )]
    winner: Option<Color>,
}

impl Generate {
    pub fn run(self) {
        for mat in self
            .mat_or_nb_pieces
            .list_of_materials_with_recursive(self.recursive)
        {
            self.gen_one_material(mat)
        }
    }

    fn gen_one_material(&self, mat: Material) {
        for winner in self
            .winner
            .map(|w| vec![w])
            .unwrap_or_else(|| Color::ALL.into())
        {
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
