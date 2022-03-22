mod generation;
mod indexer;
mod setup;

use generation::{Generator, Outcome};
pub use indexer::{from_material, index, index_unchecked, restore_from_index};
pub use setup::TbSetup;

use std::collections::HashMap;

use dhat::{Dhat, DhatAlloc};

#[global_allocator]
static ALLOCATOR: DhatAlloc = DhatAlloc;
// 3 pieces before using index At t-gmax: 19,080,095 bytes (100%) in 47 blocks (100%), avg size 405,959.47 bytes
// 4 pieces before using index At t-gmax: 610,457,858 bytes (100%) in 199 blocks (100%), avg size 3,067,627.43 bytes

fn main() {
    let _dhat = Dhat::start_heap_profiling();
    let mut gen = Generator::new("BNk"); // white king is included by default
    let setup = TbSetup::default();
    println!("gen before {:?}", gen);
    let mut q = gen.generate_positions(setup);
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
    // need to process FIRST winning positions, then losing ones.
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
    for i in 0..u8::MAX {
        if let Some(nb_win) = distrib.get(&Outcome::Win(i)) {
            println!("Win({}), {:?}", i, nb_win);
        }
    }
}
