mod generation;
mod setup;

use generation::{Generator, Outcome};
pub use setup::TbSetup;

use shakmaty::Color::{Black, White};
use std::collections::HashMap;

fn main() {
    println!("Hello, world!");
    let mut gen = Generator::default();
    // gen.winner = Black;
    let vec_pieces = vec![
        // no need for white king
        White.knight(),
        Black.rook(),
        Black.king(),
    ];
    let setup = TbSetup::default();
    println!("gen before {:?}", gen);
    let mut q = gen.generate_positions(&vec_pieces, setup);
    println!("nb pos {:?}", gen.all_pos.len());
    println!("counter {:?}", gen.counter);
    println!(
        "nb {:?} mates {:?}",
        gen.winner,
        q.winning_pos_to_process.len()
    );
    println!(
        "nb {:?} mates {:?}",
        !gen.winner,
        q.losing_pos_to_process.len()
    );
    // for rboard in gen.pos_to_process.iter() {
    //     println!("{:?}", gen.all_pos.get(rboard));
    // };
    gen.process_positions(&mut q.winning_pos_to_process);
    gen.process_positions(&mut q.losing_pos_to_process);
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();

    for (_, outcome) in gen.all_pos.iter() {
        distrib.insert(*outcome, *distrib.get(outcome).unwrap_or(&0) + 1);
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
        "From {:?} perspective, win: {:?}, draw: {:?}, lost: {:?}",
        gen.winner, win, draw, lose
    );
    println!("Distribution {:?}", distrib);
    for i in 0..30 {
        println!("Win({}), {:?}", i, distrib.get(&Outcome::Win(i)));
    }
}
