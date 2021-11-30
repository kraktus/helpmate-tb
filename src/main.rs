mod generation;
mod setup;

use generation::Generator;
pub use setup::TbSetup;

use shakmaty::Color::{Black, White};

fn main() {
    println!("Hello, world!");
    let mut gen = Generator::default();
    let mut vec_pieces = vec![Black.king(), White.queen(), White.king()];
    let setup = TbSetup::default();
    println!("gen before {:?}", gen);
    gen.generate_positions(&mut vec_pieces, setup);
    //println!("gen after {:?}", gen);
}
