mod generation;
mod indexer;
mod indexer_syzygy;
mod material;

use generation::{Generator, Outcome};
pub use indexer::{index, index_unchecked, restore_from_index};
pub use indexer_syzygy::{Pieces, Table};
pub use material::Material;

use std::collections::HashMap;

use dhat::{Dhat, DhatAlloc};

#[global_allocator]
static ALLOCATOR: DhatAlloc = DhatAlloc;
// 3 pieces before using index At t-gmax: 19,080,095 bytes (100%) in 47 blocks (100%), avg size 405,959.47 bytes
// 4 pieces before using index At t-gmax: 610,457,858 bytes (100%) in 199 blocks (100%), avg size 3,067,627.43 bytes

fn main() {
    let _dhat = Dhat::start_heap_profiling();
    let mut gen = Generator::new("KBNvK");
    let mut q = gen.generate_positions();
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
    // need to process FIRST winning positions, then losing ones.
    gen.process_positions(&mut q.winning_pos_to_process);
    gen.process_positions(&mut q.losing_pos_to_process);
    let mut draw = 0;
    let mut win = 0;
    let mut lose = 0;
    let mut distrib: HashMap<Outcome, u64> = HashMap::new();

    for by_color_outcome in gen.all_pos.iter() {
        for value in by_color_outcome.iter() {
            let outcome: Outcome = (*value).into();
            distrib.insert(outcome, *distrib.get(&outcome).unwrap_or(&0) + 1);
            match outcome {
                Outcome::Draw => draw += 1,
                Outcome::Win(_) => win += 1,
                Outcome::Lose(_) => lose += 1,
                Outcome::Unknown => (),
            }
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
