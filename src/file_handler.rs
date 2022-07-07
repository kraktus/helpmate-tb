use itertools::Itertools as _;
use positioned_io::RandomAccessFile;
use shakmaty::Color::{Black, White};
use shakmaty::Role::King;

use crate::{EncoderDecoder, Material};

struct FileHandler(RandomAccessFile);

impl FileHandler {
    pub fn new(mat: &Material) -> Self {
        let nb_pieces = mat.count();
        // TODO handle promotion
        let downstream_materials_iter = mat
            .pieces()
            .into_iter()
            .filter(|p| p.role != King)
            .combinations(nb_pieces - 1)
            .map(|subset_pieces_wo_kings| {
                subset_pieces_wo_kings
                    .into_iter()
                    .chain([Black.king(), White.king()].into_iter())
            })
            .map(Material::from_iter);

        todo!()
    }
}
