use std::io::{self, ErrorKind::InvalidData, Write};

#[cfg(feature = "cached")]
use cached::proc_macro::cached;
use deku::bitvec::BitView;
use deku::{ctx::Limit, prelude::*};
use log::trace;
use positioned_io::ReadAt;
use retroboard::shakmaty::ByColor;
use zstd::stream::{decode_all, encode_all};

use crate::{IndexWithTurn, Outcome};
use crate::{MaterialWinner, OutcomeU8, Outcomes, Report, ReportU8, Reports, ReportsSlice};

// in bytes, the size of the uncompressed block we want
const BLOCK_SIZE: usize = 500 * 1_000_000;

// number of elements we take from `outcomes`
// We want the uncompressed size of a block to be ~500Mb (arbitrary size)
// considering each elements takes 2byte
const BLOCK_ELEMENTS: usize = BLOCK_SIZE / 2;

// in bytes, the size of the cache in RAM when probing
// ideally should be set by the user, but for now good as is
#[cfg(feature = "cached")]
const CACHE_SIZE: usize = 4 * 1_000_000_000;

// max number of elements we can set in the cache for it not to exceed `CACHE_SIZE`
#[cfg(feature = "cached")]
const CACHE_ELEMENTS: usize = CACHE_SIZE / BLOCK_SIZE;

/// Deku compatible struct
#[derive(Debug, Copy, Clone, PartialEq, DekuRead, DekuWrite, Eq)]
struct RawOutcome {
    black: u8,
    white: u8,
}

impl From<&ByColor<ReportU8>> for RawOutcome {
    // TODO make this more efficient
    fn from(c: &ByColor<ReportU8>) -> Self {
        Self {
            black: OutcomeU8::from(Report::from(c.black).outcome()).as_raw_u8(),
            white: OutcomeU8::from(Report::from(c.white).outcome()).as_raw_u8(),
        }
    }
}

impl From<RawOutcome> for ByColor<OutcomeU8> {
    fn from(c: RawOutcome) -> Self {
        Self {
            black: OutcomeU8::from_raw_u8(c.black)
                .expect("Compression to be sound and keep outcome as u7"),
            white: OutcomeU8::from_raw_u8(c.white)
                .expect("Compression to be sound and keep outcome as u7"),
        }
    }
}

impl From<&RawOutcome> for ByColor<OutcomeU8> {
    fn from(c: &RawOutcome) -> Self {
        (*c).into()
    }
}

#[derive(Debug)]
pub struct EncoderDecoder<T> {
    inner: T,
}

impl<T> EncoderDecoder<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[inline]
fn to_u64(x: usize) -> u64 {
    x.try_into().unwrap()
}

impl<T: Write> EncoderDecoder<T> {
    pub fn compress(&mut self, outcomes: &Reports) -> io::Result<()> {
        for (i, elements) in outcomes.chunks(BLOCK_ELEMENTS).enumerate() {
            let block = Block::new(elements, BLOCK_ELEMENTS * i)?;
            self.inner.write_all(&block.to_bytes().unwrap())?;
        }
        Ok(())
    }
}

impl<T: ReadAt> EncoderDecoder<T> {
    fn read_block_header(&self, byte_offset: u64) -> io::Result<BlockHeader> {
        let mut header_buf: [u8; BlockHeader::BYTE_SIZE] = [0; BlockHeader::BYTE_SIZE];
        self.inner.read_exact_at(byte_offset, &mut header_buf)?;
        from_bytes_exact::<BlockHeader>(&header_buf)
    }

    fn read_block(&self, byte_offset: u64) -> io::Result<Block> {
        let block_header = self.read_block_header(byte_offset)?;
        trace!(
            "size_including_headers {:?}",
            block_header.size_including_headers()
        );
        let mut block_buf: Vec<u8> = vec![0; block_header.size_including_headers()];
        self.inner.read_exact_at(byte_offset, &mut block_buf)?;
        from_bytes_exact::<Block>(&block_buf)
    }

    pub fn outcome_of(&self, idx_with_turn: IndexWithTurn) -> io::Result<Outcome> {
        self.internal_outcome_of(None, idx_with_turn)
    }

    #[cfg(feature = "cached")]
    pub fn outcome_of_cached(
        &self,
        mat_win: MaterialWinner,
        idx_with_turn: IndexWithTurn,
    ) -> io::Result<Outcome> {
        self.internal_outcome_of(Some(mat_win), idx_with_turn)
    }

    pub fn internal_outcome_of(
        &self,
        _mat_win: Option<MaterialWinner>,
        idx_with_turn: IndexWithTurn,
    ) -> io::Result<Outcome> {
        let mut byte_offset = 0;
        loop {
            match self.read_block_header(byte_offset) {
                Ok(block_header) if block_header.idx_is_in_block(idx_with_turn.idx) => {
                    return self
                        .read_block(byte_offset)
                        .and_then(|block| {
                            #[cfg(feature = "cached")]
                            let outcome = block.get_outcome_cached(
                                _mat_win.expect(
                                    "internal_outcome_of: mat_win necessary to create cache key",
                                ),
                                idx_with_turn.idx,
                            );
                            #[cfg(not(feature = "cached"))]
                            let outcome = block.get_outcome(idx_with_turn.idx);
                            outcome
                        })
                        .map(|bc| *bc.get(idx_with_turn.turn))
                        .map(Outcome::from)
                }
                Ok(block_header) => {
                    byte_offset += to_u64(block_header.size_including_headers());
                }
                // we have reached the end of the table
                Err(ref err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "index not found in table",
        ))
    }

    pub fn decompress_file(&self) -> io::Result<Outcomes> {
        let mut outcomes = Outcomes::new();
        let mut byte_offset = 0;
        loop {
            match self.read_block(byte_offset) {
                Ok(block) => {
                    byte_offset += to_u64(block.header.size_including_headers());
                    outcomes.extend(block.decompress_outcomes()?);
                }
                // we have reached the end of the table
                Err(ref err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }
        Ok(outcomes)
    }
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq, Hash, Clone, Copy)]
struct BlockHeader {
    pub index_from: u64, // inclusive
    pub index_to: u64,   // exclusive
    pub block_size: u64, // number of bytes the actual size of the block (excluding the headers). Should be close to `BLOCK_SIZE` / 10, except for the last block
}

impl BlockHeader {
    // TODO replace by BitSize::of::<BlockHeader>() which is now const
    const BYTE_SIZE: usize = 8 * 3; // const instead of using BitSize::of::<BlockHeader>() for speed I guess

    pub fn size_including_headers(&self) -> usize {
        Self::BYTE_SIZE + self.block_size as usize
    }

    pub fn idx_is_in_block(&self, idx: u64) -> bool {
        self.index_from <= idx && idx < self.index_to
    }

    pub const fn nb_elements(&self) -> usize {
        (self.index_to - self.index_from) as usize
    }
}

// Empty wrap because `deku` does not implement `DekuWrite` for Vec<T: DekuWrite>
#[derive(Debug, PartialEq, DekuWrite, Eq)]
struct RawOutcomes(pub Vec<RawOutcome>);

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq, Hash)]
struct Block {
    header: BlockHeader,
    #[deku(count = "header.block_size")]
    pub compressed_outcomes: Vec<u8>, // compressed bytes of `Outcomes`
}

impl Block {
    pub fn new(outcomes: ReportsSlice, index_from_usize: usize) -> io::Result<Self> {
        let index_from = to_u64(index_from_usize);
        let index_to = to_u64(index_from_usize + outcomes.len());
        trace!("turning into raw outcomes");
        let raw_outcomes = RawOutcomes(outcomes.iter().map(RawOutcome::from).collect());

        trace!("turning raw outcomes into bytes");
        let raw_outcomes_bytes = raw_outcomes.to_bytes().unwrap();
        trace!("Compressing block");
        encode_all(raw_outcomes_bytes.as_slice(), 21).map(|compressed_outcomes| {
            let block_size = to_u64(compressed_outcomes.len());
            Self {
                header: BlockHeader {
                    index_from,
                    index_to,
                    block_size,
                },
                compressed_outcomes,
            }
        })
    }

    #[cfg(not(feature = "cached"))]
    fn get_outcome(&self, idx: u64) -> io::Result<ByColor<OutcomeU8>> {
        self.internal_get_outcome(None, idx)
    }

    #[cfg(feature = "cached")]
    pub fn get_outcome_cached(
        &self,
        mat_win: MaterialWinner,
        idx: u64,
    ) -> io::Result<ByColor<OutcomeU8>> {
        self.internal_get_outcome(Some(mat_win), idx)
    }

    fn internal_get_outcome(
        &self,
        _mat_win: Option<MaterialWinner>,
        idx: u64,
    ) -> io::Result<ByColor<OutcomeU8>> {
        debug_assert!(self.header.idx_is_in_block(idx));
        let block_idx = (idx.checked_sub(self.header.index_from))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Min index of the block superior to index input",
                )
            })
            .map(|idx_u64| idx_u64 as usize)?;

        #[cfg(feature = "cached")]
        let decompressed_outcomes = decompress_outcomes_cached(
            _mat_win.expect("not material winner to set cache key"),
            &self,
        );
        #[cfg(not(feature = "cached"))]
        let decompressed_outcomes = self.decompress_outcomes();
        decompressed_outcomes.and_then(|outcomes| {
            outcomes
                .get(block_idx)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Index not found in the block")
                })
                .copied()
        })
    }

    pub fn decompress_outcomes(&self) -> io::Result<Outcomes> {
        decode_all(self.compressed_outcomes.as_slice()).and_then(|decompressed_outcomes_bytes| {
            Vec::<RawOutcome>::read(
                decompressed_outcomes_bytes.view_bits(),
                Limit::new_count(self.header.nb_elements()),
            )
            .map_err(|e| io::Error::new(InvalidData, e))
            .map(|(inner_rest, raw_outcomes)| {
                assert!(inner_rest.is_empty());
                raw_outcomes
                    .into_iter()
                    .map(<ByColor<OutcomeU8>>::from)
                    .collect()
            })
        })
    }
}

#[cfg(feature = "cached")]
#[cached(result = true,
    // A block header is unique to a block given a material configuration and a winner
    type = "cached::SizedCache<(MaterialWinner, BlockHeader), Outcomes>",
    create = "{ cached::SizedCache::with_size(CACHE_ELEMENTS) }",
    convert = "{ (_mat_win.clone(), block.header) }")]
fn decompress_outcomes_cached(_mat_win: MaterialWinner, block: &Block) -> io::Result<Outcomes> {
    block.decompress_outcomes()
}

fn from_bytes_exact<'a, T: deku::DekuContainerRead<'a>>(buf: &'a [u8]) -> io::Result<T> {
    let ((byte_not_read, bit_offset), t) =
        T::from_bytes((buf, 0)).map_err(|e| io::Error::new(InvalidData, e))?;
    assert!(byte_not_read.is_empty()); // since we read the exact number of byte needed to build the struct, there should be no byte left.
    assert_eq!(bit_offset, 0); // there should never be **bit** offset neither when reader the header or after it.
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deku::ctx::BitSize;

    const DUMMY_NUMBER: usize = 10000;

    fn gen_reports(nb: usize) -> Reports {
        let mut outcomes = Reports::with_capacity(nb);
        let mut j: u8 = 0;
        let mut x: u16 = 0;
        for _i in 0..nb {
            j = j.checked_add(1).unwrap_or(0);
            x = x.checked_add(1).unwrap_or(0);
            if x == 0 {
                // println!("{i}");
            }
            let report_u8 = ReportU8::from_raw_u8(j);
            outcomes.push(ByColor {
                black: report_u8,
                white: report_u8,
            })
        }
        outcomes
    }

    fn dummy_reports() -> Reports {
        gen_reports(DUMMY_NUMBER)
    }

    fn into_outcomes(reports: Reports) -> Outcomes {
        reports
            .into_iter()
            .map(|bc| bc.map(|x| OutcomeU8::from(Report::from(x).outcome())))
            .collect()
    }

    #[test]
    fn test_block_header_size() {
        let test = BlockHeader {
            index_from: 0,
            index_to: 1,
            block_size: 0,
        };
        assert_eq!(BlockHeader::BYTE_SIZE, test.to_bytes().unwrap().len());
        assert_eq!(
            BitSize::of::<BlockHeader>(),
            BitSize(BlockHeader::BYTE_SIZE * 8),
        )
    }

    #[cfg(not(miri))]
    #[test]
    fn test_block_byte_serialisation() {
        let block = Block::new(&dummy_reports(), 0).unwrap();
        assert_eq!(
            block.to_bytes().unwrap().len(),
            block.header.size_including_headers()
        );
        println!("{:?}", block.header.size_including_headers());
        let block_2 = from_bytes_exact::<Block>(&block.to_bytes().unwrap()).unwrap();
        assert_eq!(block, block_2);
    }

    #[cfg(not(miri))]
    #[test]
    fn test_outcome_decompression() {
        let reports = dummy_reports();
        let block = Block::new(&reports, 0).unwrap();
        assert_eq!(block.decompress_outcomes().unwrap(), into_outcomes(reports));
    }

    #[cfg(not(miri))]
    #[cfg(not(feature = "cached"))]
    #[test]
    fn test_outcome_partial_decompression() {
        let reports = gen_reports(200);
        let offset = 100;
        let block = Block::new(&reports, offset).unwrap();
        for (i, report) in reports.into_iter().enumerate() {
            assert_eq!(
                block.get_outcome((i + offset) as u64).unwrap(),
                report.map(|x| OutcomeU8::from(Report::from(x).outcome()))
            );
        }
    }

    #[cfg(not(miri))]
    #[test]
    fn test_block_compression_soundness() {
        let reports = dummy_reports();
        let mut encoder = EncoderDecoder::new(Vec::<u8>::new());
        encoder.compress(&reports).expect("compression failed");
        let decompressed = encoder
            .read_block(0)
            .expect("block retrieval failed")
            .decompress_outcomes()
            .expect("decompression failed");
        assert_eq!(into_outcomes(reports), decompressed)
    }

    // Too slow even in release mode! With debug information
    // #[ignore = "too slow to be enabled by default"]
    // #[test]
    // fn test_file_compression_soundness() {
    //     let reports = gen_reports(BLOCK_ELEMENTS * 2 + BLOCK_ELEMENTS / 2);
    //     let mut encoder = EncoderDecoder::new(Vec::<u8>::new());
    //     encoder.compress(&reports).expect("compression failed");
    //     let decompressed = encoder.decompress_file().unwrap();
    //     assert_eq!(into_outcomes(reports), decompressed)
    // }
}
