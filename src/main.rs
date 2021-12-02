mod generation;
mod setup;

use generation::{Generator, Outcome, Queue};
pub use setup::TbSetup;

use shakmaty::Color::{Black, White};

fn main() {
    println!("Hello, world!");
    let mut gen = Generator::default();
    let mut q = Queue::default();
    let mut vec_pieces = vec![
        White.king(),
        White.bishop(),
        White.knight(),
        Black.king(),
        Black.knight(),
    ];
    let setup = TbSetup::default();
    println!("gen before {:?}", gen);
    gen.generate_positions(&mut vec_pieces, setup, &mut q);
    println!("nb pos {:?}", gen.all_pos.len());
    println!("nb white mates {:?}", q.winning_pos_to_process.len());
    println!("nb black mates {:?}", q.losing_pos_to_process.len());
    // for rboard in gen.pos_to_process.iter() {
    //     println!("{:?}", gen.all_pos.get(rboard));
    // };
    gen.process_positions(&mut q.winning_pos_to_process);
    gen.process_positions(&mut q.losing_pos_to_process);
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;

    for (rboard, outcome) in gen.all_pos.iter() {
        match outcome {
            Outcome::Draw => {
                draw += 1;
                //println!("{:?}", rboard)
            }
            Outcome::Win(_) => win += 1,
            Outcome::Lose(_) => lose += 1,
            Outcome::Unknown => todo!(),
        }
    }
    println!(
        "From white perspective, win: {:?}, draw: {:?}, lost: {:?}",
        win, draw, lose
    );
}
