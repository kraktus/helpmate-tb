use std::io::{self, ErrorKind::InvalidData, Write};

use deku::bitvec::{BitSlice, Msb0, BitView};
use deku::{ctx::Limit, prelude::*};
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

// TODO replace by inlined function
macro_rules! to_u64 {
    ($expression:expr) => {
        u64::try_from($expression).unwrap()
    };
}

impl<T: Write> EncoderDecoder<T> {
    pub fn compress(&mut self, outcomes: &Outcomes) -> io::Result<()> {
        Ok(
            for (i, elements) in outcomes.chunks(BLOCK_ELEMENTS).enumerate() {
                let index_from = to_u64!(BLOCK_ELEMENTS * i);
                let block = Block::new(elements, index_from)?;
                self.inner.write_all(&dbg!(block.to_bytes().unwrap()))?;
            },
        )
    }
}

impl<T: ReadAt> EncoderDecoder<T> {
    fn decompress_block_header(&self, byte_offset: u64) -> io::Result<BlockHeader> {
        let mut header_buf: [u8; BlockHeader::BYTE_SIZE] = [0; BlockHeader::BYTE_SIZE];
        self.inner.read_exact_at(byte_offset, &mut header_buf)?;
        dbg!(from_bytes_exact::<BlockHeader>(&header_buf))
    }

    fn decompress_block(&self, byte_offset: u64) -> io::Result<Block> {
        let block_header = self.decompress_block_header(byte_offset)?;
        println!(
            "size_including_headers {:?}",
            block_header.size_including_headers()
        );
        let mut block_buf: Vec<u8> = Vec::with_capacity(block_header.size_including_headers()); //vec![0; block_header.size_including_headers()]; // we read the header a second time but not a big deal
        for _ in 0..block_header.size_including_headers() {
            block_buf.push(0);
        }
        //self.inner.read_exact_at(byte_offset, &mut block_buf)?; // comment out to get (signal: 11, SIGSEGV: invalid memory reference)
        dbg!(&block_buf);
        Ok(
            // DEBUG
            Block {
                header: block_header,
                compressed_outcome: Vec::new(),
            },
        )
        // println!("{block_buf:?}");
        // Block::new(&[ByColor {white: 0, black: 0}], 0, 1) // DEBUG
        // //
        //from_bytes_exact::<Block>(&block_buf)
    }

    fn decompress(&self) -> io::Result<Outcomes> {
        let mut outcomes = Outcomes::new();
        let mut byte_offset = 0;
        loop {
            match self.decompress_block(byte_offset) {
                Ok(block) => {
                    byte_offset += block.header.size_including_headers() as u64;
                    outcomes.extend(block.decompress_outcomes()?);
                }
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => break, // or UnexpectedEof?
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
#[derive(Debug, PartialEq, DekuWrite, Eq)]
struct Block {
    header: BlockHeader,
    #[deku(reader = "Block::deku_read(deku::output, &self.header)")]
    pub outcomes: Outcomes, // compressed when serializing, decompressed at de-serializing
}

impl Block {
    pub fn new(elements: OutcomesSlice, index_from: u64) -> io::Result<Block> {
        let index_to = index_from + to_u64!(elements.len());
        let block_elements: Vec<u8> = elements
            .iter()
            .flat_map(|c| RawOutcome::from(c).to_bytes().unwrap())
            .collect();
        encode_all(block_elements.as_slice(), 21).map(|compressed_outcome| {
            let block_size = to_u64!(compressed_outcome.len());
            println!(
                "Compression ratio of the block {:?}",
                to_u64!(block_elements.len()) / block_size
            );
            Self {
                header: dbg!(BlockHeader {
                    index_from,
                    index_to,
                    block_size,
                }),

                compressed_outcome,
            }
        })
    }

    fn deku_read<'a>(
        rest: &'a BitSlice<Msb0, u8>,
        header: &'a BlockHeader,
    ) -> Result<(&'a BitSlice<Msb0, u8>, Outcomes), DekuError> {
        let nb_elements = header.nb_elements();
        Vec::<u8>::read(rest, Limit::new_count(header.block_size as usize)).and_then(
            |(rest_2, compressed_outcomes_bytes)| {
                decode_all(compressed_outcomes_bytes.as_slice())
                    .map_err(|err| DekuError::Parse(format!("{err:?}")))
                    .and_then(|decompressed_outcomes_bytes| {
                        Vec::<RawOutcome>::read(
                            decompressed_outcomes_bytes.view_bits(),
                            Limit::new_count(nb_elements),
                        )
                        .map(|(inner_rest, raw_outcomes)| {
                            (rest_2, raw_outcomes
                                .into_iter()
                                .map(<ByColor<u8>>::from)
                                .collect())
                        })
                    })
            },
        )
    }

    // pub fn decompress_outcomes(&self) -> io::Result<Outcomes> {
    //     from_bytes_exact::<RawOutcomes>(&decode_all(self.compressed_outcome.as_slice())?)
    //         .map(Outcomes::from)
    // }
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

    fn dummy_outcomes() -> Outcomes {
        let mut outcomes = Outcomes::with_capacity(DUMMY_NUMBER);
        for i in 0..DUMMY_NUMBER {
            let j = u8::try_from(i % 256).unwrap();
            outcomes.push(ByColor { black: j, white: j })
        }
        outcomes
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
        assert_eq!(block.decompress_outcomes().unwrap(), outcomes, "not equal");
    }

    #[test]
    fn test_compression_soundness() {
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
}
