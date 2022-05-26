use std::io::{self, ErrorKind::InvalidData, Write};

use deku::prelude::*;
use positioned_io::ReadAt;
use shakmaty::ByColor;
use zstd::stream::write::{Decoder as ZstdDecoder, Encoder as ZstdEncoder};

use crate::Outcomes;

// in byte, the size of the uncompressed block we want
const BLOCK_SIZE: usize = 500 * 1000000;

// number of elements we take from `outcomes`
// We want the uncompressed size of a block to be ~500Mb (arbitrary size)
// considering each elements takes 2byte
const BLOCK_ELEMENTS: usize = BLOCK_SIZE / 2;

#[derive(Debug, Clone, PartialEq, DekuRead, DekuWrite, Eq)]
struct OutcomeByColor {
    black: u8,
    white: u8,
}

impl From<&ByColor<u8>> for OutcomeByColor {
    fn from(c: &ByColor<u8>) -> Self {
        Self {
            black: c.black,
            white: c.white,
        }
    }
}

impl From<OutcomeByColor> for ByColor<u8> {
    fn from(c: OutcomeByColor) -> Self {
        Self {
            black: c.black,
            white: c.white,
        }
    }
}

pub struct EncoderDecoder<T> {
    inner: T,
}

impl<T> EncoderDecoder<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: Write> EncoderDecoder<T> {
    pub fn compress(&mut self, outcomes: &Outcomes) -> io::Result<()> {
        Ok(for (i, elements) in outcomes.chunks(BLOCK_ELEMENTS).enumerate() {
            let index_from = u64::try_from(BLOCK_ELEMENTS * i).unwrap();
            let block_size = u64::try_from(elements.len()).unwrap();
            let index_to = index_from + block_size;
            let block_elements: Vec<u8> = elements
                .iter()
                .flat_map(|c| OutcomeByColor::from(c).to_bytes().unwrap())
                .collect();
            let compressed_outcome_writer: Vec<u8> = Vec::with_capacity(BLOCK_ELEMENTS); // writing in memory is much faster than in a file
            let mut encoder = ZstdEncoder::new(compressed_outcome_writer, 21)?; // set compression level to the maximum
            let compressed_block_size = encoder.write(&block_elements)?;
            println!(
                "Compression ratio of the block {:?}",
                block_size / u64::try_from(compressed_block_size).unwrap()
            );
            let compressed_outcome = encoder.finish()?;
            let block = Block {
                header: BlockHeader {
                    index_from,
                    index_to,
                    block_size,
                },

                compressed_outcome,
            };
            self.inner.write_all(&block.to_bytes().unwrap())?;
        })
    }
}

impl<T: ReadAt> EncoderDecoder<T> {
    fn decompress_block_header(&self, byte_offset: u64) -> io::Result<BlockHeader> {
        let mut header_buf: [u8; BlockHeader::BYTE_SIZE] = [0; BlockHeader::BYTE_SIZE];
        self.inner.read_exact_at(byte_offset, &mut header_buf)?;
        from_bytes_exact::<BlockHeader>(&header_buf)
    }

    fn decompress_block(&self, byte_offset: u64) -> io::Result<Outcomes> {
        let block_header = self.decompress_block_header(byte_offset)?;
        let mut block_buf: Vec<u8> =
            Vec::with_capacity(BlockHeader::BYTE_SIZE + block_header.block_size as usize); // we read the header a second time but not a big deal
        self.inner.read_exact_at(byte_offset, &mut block_buf)?;
        from_bytes_exact::<Block>(&block_buf)?.decompress_outcomes()
    }
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq)]
struct BlockHeader {
    pub index_from: u64, // inclusive
    pub index_to: u64,   // exclusive
    pub block_size: u64, // number of bytes the actual size of the block. Should be close to `BLOCK_SIZE` / 10, except for the last block
}

impl BlockHeader {
    const BYTE_SIZE: usize = 8 * 3;
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq)]
struct Block {
    header: BlockHeader,
    #[deku(count = "header.block_size")]
    pub compressed_outcome: Vec<u8>, // compressed version of `Vec<ByColor<u8>`
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Eq)]
struct RawOutcomes([OutcomeByColor; BLOCK_ELEMENTS]);

impl From<RawOutcomes> for Outcomes {
    fn from(raw_outcomes: RawOutcomes) -> Self {
        raw_outcomes
            .0
            .into_iter()
            .map(<ByColor<u8>>::from)
            .collect()
    }
}

impl Block {
    pub fn decompress_outcomes(&self) -> io::Result<Outcomes> {
        let mut uncompressed_outcome_writer: Vec<u8> = Vec::with_capacity(BLOCK_SIZE);
        let mut decoder = ZstdDecoder::new(&mut uncompressed_outcome_writer)?;
        decoder.write_all(&self.compressed_outcome)?;
        decoder.flush()?;
        from_bytes_exact::<RawOutcomes>(decoder.into_inner()).map(Outcomes::from)
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

    #[test]
    fn test_block_header_size() {
        assert_eq!(Size::of::<BlockHeader>(), Size::Bits(BlockHeader::BYTE_SIZE * 8))
    }


    #[test]
    fn test_compression_soundness() {
        // let outcomes: Outcomes = vec![ByColor { black: 134, white: 137 }, ByColor { black: 134, white: 255 }, ByColor { black: 134, white: 137 }, ByColor { black: 136, white: 137 }, ByColor { black: 134, white: 137 }, ByColor { black: 134, white: 255 }, ByColor { black: 134, white: 137 }, ByColor { black: 134, white: 137 }];
        // let writer: Vec<u8> = Vec::new();
        // let encoder = EncoderDecoder::new(writer);

    }
}
