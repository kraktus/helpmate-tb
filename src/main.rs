mod generation;
mod setup;

use generation::{Generator, Outcome};
pub use setup::TbSetup;

use shakmaty::Color::{Black, White};

fn main() {
    println!("Hello, world!");
    let mut gen = Generator::default();
    let mut vec_pieces = vec![White.king(), White.queen(), Black.king()];
    let setup = TbSetup::default();
    println!("gen before {:?}", gen);
    gen.generate_positions(&mut vec_pieces, setup);
    println!("nb pos {:?}", gen.all_pos.len());
    println!("nb mates {:?}", gen.pos_to_process.len());
    for rboard in gen.pos_to_process.iter() {
        //println!("{:?}", gen.all_pos.get(rboard));
    };
    gen.process_positions();
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;

    for (rboard, outcome) in gen.all_pos.iter() {
        match outcome {
            Outcome::Draw => draw +=1,
            Outcome::Win(_) if rboard.retro_turn() == Black => win += 1,
            Outcome::Win(_) => lose +=1,
            Outcome::Lose(_) if rboard.retro_turn() == White => win += 1,
            Outcome::Lose(_) => lose +=1,
        }
    }
    println!("From white perspective, win: {:?}, draw: {:?}, lost: {:?}", win, draw, lose);
}
