use std::io::{self, ErrorKind::InvalidData, Write};

use deku::bitvec::BitView;
use deku::{ctx::Limit, prelude::*};
use log::trace;
use positioned_io::ReadAt;
use shakmaty::ByColor;
use zstd::stream::{decode_all, encode_all};

use crate::{Outcomes, OutcomesSlice};

// in byte, the size of the uncompressed block we want
const BLOCK_SIZE: usize = 500 * 1000000;

// number of elements we take from `outcomes`
// We want the uncompressed size of a block to be ~500Mb (arbitrary size)
// considering each elements takes 2byte
const BLOCK_ELEMENTS: usize = BLOCK_SIZE / 2;

/// Deku compatible struct
#[derive(Debug, Clone, PartialEq, DekuRead, DekuWrite, Eq)]
struct RawOutcome {
    black: u8,
    white: u8,
}

impl From<&ByColor<u8>> for RawOutcome {
    fn from(c: &ByColor<u8>) -> Self {
        Self {
            black: c.black,
            white: c.white,
        }
    }
}

impl From<RawOutcome> for ByColor<u8> {
    fn from(c: RawOutcome) -> Self {
        Self {
            black: c.black,
            white: c.white,
        }
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
    pub fn compress(&mut self, outcomes: &Outcomes) -> io::Result<()> {
        Ok(
            for (i, elements) in outcomes.chunks(BLOCK_ELEMENTS).enumerate() {
                let block = Block::new(elements, BLOCK_ELEMENTS * i)?;
                self.inner.write_all(&block.to_bytes().unwrap())?;
            },
        )
    }
}

impl<T: ReadAt> EncoderDecoder<T> {
    fn decompress_block_header(&self, byte_offset: u64) -> io::Result<BlockHeader> {
        let mut header_buf: [u8; BlockHeader::BYTE_SIZE] = [0; BlockHeader::BYTE_SIZE];
        self.inner.read_exact_at(byte_offset, &mut header_buf)?;
        from_bytes_exact::<BlockHeader>(&header_buf)
    }

    fn decompress_block(&self, byte_offset: u64) -> io::Result<Block> {
        let block_header = self.decompress_block_header(byte_offset)?;
        trace!(
            "size_including_headers {:?}",
            block_header.size_including_headers()
        );
        let mut block_buf: Vec<u8> = vec![0; block_header.size_including_headers()];
        self.inner.read_exact_at(byte_offset, &mut block_buf)?;
        from_bytes_exact::<Block>(&block_buf)
    }

    pub fn decompress_file(&self) -> io::Result<Outcomes> {
        let mut outcomes = Outcomes::new();
        let mut byte_offset = 0;
        loop {
            match self.decompress_block(byte_offset) {
                Ok(block) => {
                    byte_offset += block.header.size_including_headers() as u64;
                    outcomes.extend(block.decompress_outcomes()?);
                }
                // we have reached the end of the file
                Err(ref err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }
        Ok(outcomes)
    }
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq)]
struct BlockHeader {
    pub index_from: u64, // inclusive
    pub index_to: u64,   // exclusive
    pub block_size: u64, // number of bytes the actual size of the block (excluding the headers). Should be close to `BLOCK_SIZE` / 10, except for the last block
}

impl BlockHeader {
    const BYTE_SIZE: usize = 8 * 3;

    pub fn size_including_headers(&self) -> usize {
        Self::BYTE_SIZE + self.block_size as usize
    }

    pub const fn nb_elements(&self) -> usize {
        (self.index_to - self.index_from) as usize
    }
}

// Empty wrap because `deku` does not implement `DekuWrite` for Vec<T: DekuWrite>
#[derive(Debug, PartialEq, DekuWrite, Eq)]
struct RawOutcomes(pub Vec<RawOutcome>);

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq)]
struct Block {
    header: BlockHeader,
    #[deku(count = "header.block_size")]
    pub compressed_outcomes: Vec<u8>, // compressed bytes of `Outcomes`
}

impl Block {
    pub fn new(outcomes: OutcomesSlice, index_from_usize: usize) -> io::Result<Self> {
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

    pub fn decompress_outcomes(&self) -> io::Result<Outcomes> {
        decode_all(self.compressed_outcomes.as_slice()).and_then(|decompressed_outcomes_bytes| {
            Vec::<RawOutcome>::read(
                decompressed_outcomes_bytes.view_bits(),
                Limit::new_count(self.header.nb_elements()),
            )
            .map_err(|e| io::Error::new(InvalidData, e))
            .map(|(inner_rest, raw_outcomes)| {
                assert!(inner_rest.is_empty());
                raw_outcomes.into_iter().map(<ByColor<u8>>::from).collect()
            })
        })
    }
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
    use deku::ctx::Size;

    const DUMMY_NUMBER: usize = 10000;

    fn gen_outcomes(nb: usize) -> Outcomes {
        let mut outcomes = Outcomes::with_capacity(nb);
        let mut j: u8 = 0;
        let mut x: u16 = 0;
        for _i in 0..nb {
            j = j.checked_add(1).unwrap_or(0);
            x = x.checked_add(1).unwrap_or(0);
            if x == 0 {
                // println!("{i}");
            }
            outcomes.push(ByColor { black: j, white: j })
        }
        outcomes
    }

    fn dummy_outcomes() -> Outcomes {
        gen_outcomes(DUMMY_NUMBER)
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
            Size::of::<BlockHeader>(),
            Size::Bits(BlockHeader::BYTE_SIZE * 8),
        )
    }

    #[test]
    fn test_block_byte_serialisation() {
        let block = Block::new(&dummy_outcomes(), 0).unwrap();
        assert_eq!(
            block.to_bytes().unwrap().len(),
            block.header.size_including_headers()
        );
        println!("{:?}", block.header.size_including_headers());
        let block_2 = from_bytes_exact::<Block>(&block.to_bytes().unwrap()).unwrap();
        assert_eq!(block, block_2);
    }

    #[test]
    fn test_outcome_decompression() {
        let outcomes = dummy_outcomes();
        let block = Block::new(&outcomes, 0).unwrap();
        assert_eq!(block.decompress_outcomes().unwrap(), outcomes);
    }

    #[test]
    fn test_block_compression_soundness() {
        let outcomes = dummy_outcomes();
        let mut encoder = EncoderDecoder::new(Vec::<u8>::new());
        encoder.compress(&outcomes).expect("compression failed");
        let decompressed = encoder
            .decompress_block(0)
            .expect("block retrieval failed")
            .decompress_outcomes()
            .expect("decompression failed");
        assert_eq!(outcomes, decompressed)
    }

    // deku is too slow with debug information to run
    // #[test]
    // fn test_file_compression_soundness() {
    //     let outcomes = gen_outcomes(BLOCK_ELEMENTS * 2 + BLOCK_ELEMENTS / 2);
    //     let mut encoder = EncoderDecoder::new(Vec::<u8>::new());
    //     encoder.compress(&outcomes).expect("compression failed");
    //     let decompressed = encoder.decompress_file().unwrap();
    //     assert_eq!(outcomes, decompressed)
    // }
}
