use std::collections::HashMap;

use positioned_io::RandomAccessFile;

use crate::{EncoderDecoder, Material, Outcomes, SideToMove, SideToMoveGetter, Table};

#[derive(Debug)]
struct FileHandler {
    pub table: Table,
    pub outcomes: Outcomes,
}


impl FileHandler {
    pub fn new(mat: &Material) -> Self {
        let raf = RandomAccessFile::open(format!("table/{:?}", mat))
            .expect("table file to be generated and accessible");
        let outcomes = EncoderDecoder::new(raf)
            .decompress_file()
            .expect("File well formated and readable");
        let table = Table::new(mat);
        Self { table, outcomes }
    }
}

#[derive(Debug)]
pub struct TableBase(HashMap<Material, FileHandler>);

impl TableBase {
    pub fn new(mat: &Material) -> Self {
        Self(
            mat.descendants_not_draw()
                .map(|m| {
                    let file_handler = FileHandler::new(&m);
                    (m, file_handler)
                })
                .collect(),
        )
    }

    /// Returns the distance to helpmate in the descendant table, or panics
    pub fn retrieve_outcome(&self, pos: &dyn SideToMove) -> u8 {
        let mat: Material = pos.board().material().into();
        let handler = self.0.get(&mat).expect("Position to be among descendants");
        let idx = handler.table.encode(pos);
        *handler.outcomes[idx].got(pos)
    }
}
