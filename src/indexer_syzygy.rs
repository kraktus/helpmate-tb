use arrayvec::ArrayVec;
use itertools::Itertools as _;
use shakmaty::Bitboard;
use shakmaty::{File, Piece, Rank, Role, Square};

use crate::{get_info_table, Material, SideToMove};

const fn binomial(mut n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    if k > n - k {
        return binomial(n, n - k);
    }
    let mut r = 1;
    let mut d = 1;
    while d <= k {
        r = r * n / d;
        n -= 1;
        d += 1;
    }
    r
}

const MAX_PIECES: usize = 7;

/// Maps squares into the a1-d1-d4 triangle.
#[rustfmt::skip]
const TRIANGLE: [u64; 64] = [
    6, 0, 1, 2, 2, 1, 0, 6,
    0, 7, 3, 4, 4, 3, 7, 0,
    1, 3, 8, 5, 5, 8, 3, 1,
    2, 4, 5, 9, 9, 5, 4, 2,
    2, 4, 5, 9, 9, 5, 4, 2,
    1, 3, 8, 5, 5, 8, 3, 1,
    0, 7, 3, 4, 4, 3, 7, 0,
    6, 0, 1, 2, 2, 1, 0, 6,
];

pub type Pieces = ArrayVec<Piece, MAX_PIECES>;

/// Inverse of `TRIANGLE`.
const INV_TRIANGLE: [usize; 10] = [1, 2, 3, 10, 11, 19, 0, 9, 18, 27];

/// Maps the b1-h1-h7 triangle to `0..=27`.
#[rustfmt::skip]
const LOWER: [u64; 64] = [
    28,  0,  1,  2,  3,  4,  5,  6,
     0, 29,  7,  8,  9, 10, 11, 12,
     1,  7, 30, 13, 14, 15, 16, 17,
     2,  8, 13, 31, 18, 19, 20, 21,
     3,  9, 14, 18, 32, 22, 23, 24,
     4, 10, 15, 19, 22, 33, 25, 26,
     5, 11, 16, 20, 23, 25, 34, 27,
     6, 12, 17, 21, 24, 26, 27, 35,
];

/// Used to initialize `Consts::mult_idx` and `Consts::mult_factor`.
#[rustfmt::skip]
const MULT_TWIST: [u64; 64] = [
    15, 63, 55, 47, 40, 48, 56, 12,
    62, 11, 39, 31, 24, 32,  8, 57,
    54, 38,  7, 23, 16,  4, 33, 49,
    46, 30, 22,  3,  0, 17, 25, 41,
    45, 29, 21,  2,  1, 18, 26, 42,
    53, 37,  6, 20, 19,  5, 34, 50,
    61, 10, 36, 28, 27, 35,  9, 58,
    14, 60, 52, 44, 43, 51, 59, 13,
];

/// Unused entry. Initialized to `-1`, so that most uses will cause noticable
/// overflow in debug mode.
const Z0: u64 = u64::max_value();

/// Encoding of all 461 configurations of two not-connected kings.
#[rustfmt::skip]
const KK_IDX: [[u64; 64]; 10] = [[
     Z0,  Z0,  Z0,   0,   1,   2,   3,   4,
     Z0,  Z0,  Z0,   5,   6,   7,   8,   9,
     10,  11,  12,  13,  14,  15,  16,  17,
     18,  19,  20,  21,  22,  23,  24,  25,
     26,  27,  28,  29,  30,  31,  32,  33,
     34,  35,  36,  37,  38,  39,  40,  41,
     42,  43,  44,  45,  46,  47,  48,  49,
     50,  51,  52,  53,  54,  55,  56,  57,
], [
     58,  Z0,  Z0,  Z0,  59,  60,  61,  62,
     63,  Z0,  Z0,  Z0,  64,  65,  66,  67,
     68,  69,  70,  71,  72,  73,  74,  75,
     76,  77,  78,  79,  80,  81,  82,  83,
     84,  85,  86,  87,  88,  89,  90,  91,
     92,  93,  94,  95,  96,  97,  98,  99,
    100, 101, 102, 103, 104, 105, 106, 107,
    108, 109, 110, 111, 112, 113, 114, 115,
], [
    116, 117,  Z0,  Z0,  Z0, 118, 119, 120,
    121, 122,  Z0,  Z0,  Z0, 123, 124, 125,
    126, 127, 128, 129, 130, 131, 132, 133,
    134, 135, 136, 137, 138, 139, 140, 141,
    142, 143, 144, 145, 146, 147, 148, 149,
    150, 151, 152, 153, 154, 155, 156, 157,
    158, 159, 160, 161, 162, 163, 164, 165,
    166, 167, 168, 169, 170, 171, 172, 173,
], [
    174,  Z0,  Z0,  Z0, 175, 176, 177, 178,
    179,  Z0,  Z0,  Z0, 180, 181, 182, 183,
    184,  Z0,  Z0,  Z0, 185, 186, 187, 188,
    189, 190, 191, 192, 193, 194, 195, 196,
    197, 198, 199, 200, 201, 202, 203, 204,
    205, 206, 207, 208, 209, 210, 211, 212,
    213, 214, 215, 216, 217, 218, 219, 220,
    221, 222, 223, 224, 225, 226, 227, 228,
], [
    229, 230,  Z0,  Z0,  Z0, 231, 232, 233,
    234, 235,  Z0,  Z0,  Z0, 236, 237, 238,
    239, 240,  Z0,  Z0,  Z0, 241, 242, 243,
    244, 245, 246, 247, 248, 249, 250, 251,
    252, 253, 254, 255, 256, 257, 258, 259,
    260, 261, 262, 263, 264, 265, 266, 267,
    268, 269, 270, 271, 272, 273, 274, 275,
    276, 277, 278, 279, 280, 281, 282, 283,
], [
    284, 285, 286, 287, 288, 289, 290, 291,
    292, 293,  Z0,  Z0,  Z0, 294, 295, 296,
    297, 298,  Z0,  Z0,  Z0, 299, 300, 301,
    302, 303,  Z0,  Z0,  Z0, 304, 305, 306,
    307, 308, 309, 310, 311, 312, 313, 314,
    315, 316, 317, 318, 319, 320, 321, 322,
    323, 324, 325, 326, 327, 328, 329, 330,
    331, 332, 333, 334, 335, 336, 337, 338,
], [
     Z0,  Z0, 339, 340, 341, 342, 343, 344,
     Z0,  Z0, 345, 346, 347, 348, 349, 350,
     Z0,  Z0, 441, 351, 352, 353, 354, 355,
     Z0,  Z0,  Z0, 442, 356, 357, 358, 359,
     Z0,  Z0,  Z0,  Z0, 443, 360, 361, 362,
     Z0,  Z0,  Z0,  Z0,  Z0, 444, 363, 364,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 445, 365,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 446,
], [
     Z0,  Z0,  Z0, 366, 367, 368, 369, 370,
     Z0,  Z0,  Z0, 371, 372, 373, 374, 375,
     Z0,  Z0,  Z0, 376, 377, 378, 379, 380,
     Z0,  Z0,  Z0, 447, 381, 382, 383, 384,
     Z0,  Z0,  Z0,  Z0, 448, 385, 386, 387,
     Z0,  Z0,  Z0,  Z0,  Z0, 449, 388, 389,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 450, 390,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 451,
], [
    452, 391, 392, 393, 394, 395, 396, 397,
     Z0,  Z0,  Z0,  Z0, 398, 399, 400, 401,
     Z0,  Z0,  Z0,  Z0, 402, 403, 404, 405,
     Z0,  Z0,  Z0,  Z0, 406, 407, 408, 409,
     Z0,  Z0,  Z0,  Z0, 453, 410, 411, 412,
     Z0,  Z0,  Z0,  Z0,  Z0, 454, 413, 414,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 455, 415,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 456,
], [
    457, 416, 417, 418, 419, 420, 421, 422,
     Z0, 458, 423, 424, 425, 426, 427, 428,
     Z0,  Z0,  Z0,  Z0,  Z0, 429, 430, 431,
     Z0,  Z0,  Z0,  Z0,  Z0, 432, 433, 434,
     Z0,  Z0,  Z0,  Z0,  Z0, 435, 436, 437,
     Z0,  Z0,  Z0,  Z0,  Z0, 459, 438, 439,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 460, 440,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 461,
]];

/// Encoding of a pair of identical pieces.
#[rustfmt::skip]
const PP_IDX: [[u64; 64]; 10] = [[
      0,  Z0,   1,   2,   3,   4,   5,   6,
      7,   8,   9,  10,  11,  12,  13,  14,
     15,  16,  17,  18,  19,  20,  21,  22,
     23,  24,  25,  26,  27,  28,  29,  30,
     31,  32,  33,  34,  35,  36,  37,  38,
     39,  40,  41,  42,  43,  44,  45,  46,
     Z0,  47,  48,  49,  50,  51,  52,  53,
     54,  55,  56,  57,  58,  59,  60,  61,
], [
     62,  Z0,  Z0,  63,  64,  65,  Z0,  66,
     Z0,  67,  68,  69,  70,  71,  72,  Z0,
     73,  74,  75,  76,  77,  78,  79,  80,
     81,  82,  83,  84,  85,  86,  87,  88,
     89,  90,  91,  92,  93,  94,  95,  96,
     Z0,  97,  98,  99, 100, 101, 102, 103,
     Z0, 104, 105, 106, 107, 108, 109,  Z0,
    110,  Z0, 111, 112, 113, 114,  Z0, 115,
], [
    116,  Z0,  Z0,  Z0, 117,  Z0,  Z0, 118,
     Z0, 119, 120, 121, 122, 123, 124,  Z0,
     Z0, 125, 126, 127, 128, 129, 130,  Z0,
    131, 132, 133, 134, 135, 136, 137, 138,
     Z0, 139, 140, 141, 142, 143, 144, 145,
     Z0, 146, 147, 148, 149, 150, 151,  Z0,
     Z0, 152, 153, 154, 155, 156, 157,  Z0,
    158,  Z0,  Z0, 159, 160,  Z0,  Z0, 161,
], [
    162,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 163,
     Z0, 164,  Z0, 165, 166, 167, 168,  Z0,
     Z0, 169, 170, 171, 172, 173, 174,  Z0,
     Z0, 175, 176, 177, 178, 179, 180,  Z0,
     Z0, 181, 182, 183, 184, 185, 186,  Z0,
     Z0,  Z0, 187, 188, 189, 190, 191,  Z0,
     Z0, 192, 193, 194, 195, 196, 197,  Z0,
    198,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 199,
], [
    200,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 201,
     Z0, 202,  Z0,  Z0, 203,  Z0, 204,  Z0,
     Z0,  Z0, 205, 206, 207, 208,  Z0,  Z0,
     Z0, 209, 210, 211, 212, 213, 214,  Z0,
     Z0,  Z0, 215, 216, 217, 218, 219,  Z0,
     Z0,  Z0, 220, 221, 222, 223,  Z0,  Z0,
     Z0, 224,  Z0, 225, 226,  Z0, 227,  Z0,
    228,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 229,
], [
    230,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 231,
     Z0, 232,  Z0,  Z0,  Z0,  Z0, 233,  Z0,
     Z0,  Z0, 234,  Z0, 235, 236,  Z0,  Z0,
     Z0,  Z0, 237, 238, 239, 240,  Z0,  Z0,
     Z0,  Z0,  Z0, 241, 242, 243,  Z0,  Z0,
     Z0,  Z0, 244, 245, 246, 247,  Z0,  Z0,
     Z0, 248,  Z0,  Z0,  Z0,  Z0, 249,  Z0,
    250,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 251,
], [
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 259,
     Z0, 252,  Z0,  Z0,  Z0,  Z0, 260,  Z0,
     Z0,  Z0, 253,  Z0,  Z0, 261,  Z0,  Z0,
     Z0,  Z0,  Z0, 254, 262,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0, 255,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0, 256,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 257,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 258,
], [
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 268,  Z0,
     Z0,  Z0, 263,  Z0,  Z0, 269,  Z0,  Z0,
     Z0,  Z0,  Z0, 264, 270,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0, 265,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0, 266,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0, 267,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
], [
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0, 274,  Z0,  Z0,
     Z0,  Z0,  Z0, 271, 275,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0, 272,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0, 273,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
], [
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0, 277,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0, 276,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,
     Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0,  Z0
]];

/// The a7-a5-c5 triangle.
const TEST45: Bitboard = Bitboard(0x1_0307_0000_0000);

const CONSTS: Consts = Consts::new();

struct Consts {
    mult_idx: [[u64; 10]; 5],
    mult_factor: [u64; 5],

    map_pawns: [u64; 64],
    lead_pawn_idx: [[u64; 64]; 6],
    lead_pawns_size: [[u64; 4]; 6],
}

impl Consts {
    const fn new() -> Consts {
        let mut mult_idx = [[0; 10]; 5];
        let mut mult_factor = [0; 5];

        let mut i = 0;
        while i < 5 {
            let mut s = 0;
            let mut j = 0;
            while j < 10 {
                mult_idx[i][j] = s;
                s += if i == 0 {
                    1
                } else {
                    binomial(MULT_TWIST[INV_TRIANGLE[j]], i as u64)
                };
                j += 1;
            }
            mult_factor[i] = s;
            i += 1;
        }

        let mut available_squares = 48;

        let mut map_pawns = [0; 64];
        let mut lead_pawn_idx = [[0; 64]; 6];
        let mut lead_pawns_size = [[0; 4]; 6];

        let mut lead_pawns_cnt = 1;
        while lead_pawns_cnt <= 5 {
            let mut file = 0;
            while file < 4 {
                let mut idx = 0;
                let mut rank = 1;
                while rank < 7 {
                    let sq = file + 8 * rank;
                    if lead_pawns_cnt == 1 {
                        available_squares -= 1;
                        map_pawns[sq] = available_squares;
                        available_squares -= 1;
                        map_pawns[sq ^ 0x7] = available_squares; // flip horizontal
                    }
                    lead_pawn_idx[lead_pawns_cnt][sq] = idx;
                    idx += binomial(map_pawns[sq], lead_pawns_cnt as u64 - 1);
                    rank += 1;
                }
                lead_pawns_size[lead_pawns_cnt][file] = idx;
                file += 1;
            }
            lead_pawns_cnt += 1;
        }

        Consts {
            mult_idx,
            mult_factor,
            map_pawns,
            lead_pawn_idx,
            lead_pawns_size,
        }
    }
}

/// A Syzygy table.
#[derive(Debug, Clone)]
pub struct Table {
    num_unique_pieces: u8,
    min_like_man: u8,
    files: ArrayVec<ArrayVec<GroupData, 2>, 4>,
}

/// Checks if a square is on the a1-h8 diagonal.
fn offdiag(sq: Square) -> bool {
    sq.file().flip_diagonal() != sq.rank()
}

/// Description of the encoding used for a piece configuration.
#[derive(Debug, Clone)]
pub struct GroupData {
    pieces: Pieces,
    lens: ArrayVec<usize, MAX_PIECES>,
    factors: ArrayVec<u64, { MAX_PIECES + 1 }>,
}

fn group_pieces(pieces: &Pieces) -> ArrayVec<usize, MAX_PIECES> {
    let mut result = ArrayVec::new();
    let material = Material::from_iter(pieces.clone());

    // For pawnless positions: If there are at least 3 unique pieces then 3
    // unique pieces wil form the leading group. Otherwise the two kings will
    // form the leading group.
    let first_len = if material.has_pawns() {
        0
    } else if material.unique_pieces() >= 3 {
        3
    } else if material.unique_pieces() == 2 {
        2
    } else {
        usize::from(material.min_like_man())
    };

    if first_len > 0 {
        result.push(first_len);
    }

    // The remaining identical pieces are grouped together.
    result.extend(
        pieces
            .iter()
            .skip(first_len)
            .group_by(|p| *p)
            .into_iter()
            .map(|(_, g)| g.count()),
    );

    result
}

impl GroupData {
    pub fn new(pieces: Pieces, order: [u8; 2], file: usize) -> Self {
        assert!(pieces.len() >= 2);

        let material = Material::from_iter(pieces.clone());

        // Compute group lengths.
        let lens = group_pieces(&pieces);

        // Compute a factor for each group.
        let pp = material.by_color.white.has_pawns() && material.by_color.black.has_pawns();
        let mut factors = ArrayVec::from([0; MAX_PIECES + 1]);
        factors.truncate(lens.len() + 1);
        let mut free_squares = 64 - lens[0] - if pp { lens[1] } else { 0 };
        let mut next = if pp { 2 } else { 1 };
        let mut idx = 1;
        let mut k = 0;

        while next < lens.len() || k == order[0] || k == order[1] {
            if k == order[0] {
                // Leading pawns or pieces.
                factors[0] = idx;

                if material.has_pawns() {
                    idx *= CONSTS.lead_pawns_size[lens[0]][file];
                } else if material.unique_pieces() >= 3 {
                    idx *= 31_332;
                } else if material.unique_pieces() == 2 {
                    idx *= 462;
                } else if material.min_like_man() == 2 {
                    idx *= 278;
                } else {
                    idx *= CONSTS.mult_factor[usize::from(material.min_like_man()) - 1];
                }
            } else if k == order[1] {
                // Remaining pawns.
                factors[1] = idx;
                idx *= binomial(48 - lens[0] as u64, lens[1] as u64);
            } else {
                // Remaining pieces.
                factors[next] = idx;
                idx *= binomial(free_squares as u64, lens[next] as u64);
                free_squares -= lens[next];
                next += 1;
            }
            k += 1;
        }

        factors[lens.len()] = idx;

        Self {
            pieces,
            lens,
            factors,
        }
    }
}

impl Table {
    pub fn new(material: &Material) -> Self {
        let material_info = get_info_table(material).unwrap();
        let files: ArrayVec<ArrayVec<GroupData, 2>, 4> = material_info
            .iter()
            .enumerate()
            .map(|(file, infos)| {
                infos
                    .iter()
                    .map(|side| {
                        GroupData::new(
                            ArrayVec::from_iter(side.pieces.clone().into_iter()),
                            side.order,
                            file,
                        )
                    })
                    .collect()
            })
            .collect();
        println!("files at the end {:?}", files[0][0]);
        Self {
            num_unique_pieces: material.unique_pieces(),
            min_like_man: material.min_like_man(),
            files,
        }
    }

    pub fn encode(&self, pos: &dyn SideToMove) -> usize {
        self.encode_checked(pos)
            .expect("Valid index, it not sure use `encode_checked`")
    }

    /// Given a position, determine the unique (modulo symmetries) index into
    /// the corresponding subtable.
    pub fn encode_checked(&self, pos: &dyn SideToMove) -> Result<usize, ()> {
        let key = Material::from_board(pos.board());
        let material = Material::from_iter(self.files[0][0].pieces.clone());
        let key_check = key == material || key == material.clone().into_flipped();

        if !key_check {
            println!("{:?}", &pos.board());
        }
        assert!(key_check);

        let symmetric_btm = material.is_symmetric() && pos.side_to_move().is_black();
        let black_stronger = key != material;
        let flip = symmetric_btm || black_stronger;
        let bside = pos.side_to_move().is_black() ^ flip;

        let mut squares: ArrayVec<Square, MAX_PIECES> = ArrayVec::new();
        let mut used = Bitboard(0);

        // For pawns there are subtables for each file (a, b, c, d) the
        // leading pawn can be placed on.
        let file = &self.files[if material.has_pawns() {
            let reference_pawn = self.files[0][0].pieces[0];
            assert_eq!(reference_pawn.role, Role::Pawn);
            let color = reference_pawn.color ^ flip;

            let lead_pawns = pos.board().pawns() & pos.board().by_color(color);
            used.extend(lead_pawns);
            squares.extend(
                lead_pawns
                    .into_iter()
                    .map(|sq| if flip { sq.flip_vertical() } else { sq }),
            );

            // Ensure squares[0] is the maximum with regard to map_pawns.
            for i in 1..squares.len() {
                if CONSTS.map_pawns[usize::from(squares[0])]
                    < CONSTS.map_pawns[usize::from(squares[i])]
                {
                    squares.swap(0, i);
                }
            }
            if squares[0].file() >= File::E {
                squares[0].flip_horizontal().file() as usize
            } else {
                squares[0].file() as usize
            }
        } else {
            0
        }];

        // WDL tables have subtables for each side to move.
        let side = &file[if bside { file.len() - 1 } else { 0 }];

        // DTZ tables store only one side to move. It is possible that we have
        // to check the other side (by doing a 1-ply search).
        // if T::METRIC == Metric::Dtz
        //     && side.flags.contains(Flag::STM) != bside
        //     && (!material.is_symmetric() || material.has_pawns())
        // {
        //     return Ok(None);
        // }

        // The subtable has been determined.
        //
        // So far squares has been initialized with the leading pawns.
        // Also add the other pieces.
        let lead_pawns_count = squares.len();

        for piece in side.pieces.iter().skip(lead_pawns_count) {
            let color = piece.color ^ flip;
            let square = ((pos.board().by_piece(piece.role.of(color)) & !used).first())
                .expect("Uncorrupted table (what does it mean?)");
            squares.push(if flip { square.flip_vertical() } else { square });
            used.add(square);
        }

        assert!(squares.len() >= 2);

        // Now we can compute the index according to the piece positions.
        if squares[0].file() >= File::E {
            for square in &mut squares {
                *square = square.flip_horizontal();
            }
        }

        let mut idx = if material.has_pawns() {
            let mut idx = CONSTS.lead_pawn_idx[lead_pawns_count][usize::from(squares[0])];

            squares[1..lead_pawns_count]
                .sort_unstable_by_key(|sq| CONSTS.map_pawns[usize::from(*sq)]);

            for (i, &square) in squares.iter().enumerate().take(lead_pawns_count).skip(1) {
                idx += binomial(CONSTS.map_pawns[usize::from(square)], i as u64);
            }

            idx
        } else {
            if squares[0].rank() >= Rank::Fifth {
                for square in &mut squares {
                    *square = square.flip_vertical();
                }
            }

            for i in 0..side.lens[0] {
                if squares[i].file().flip_diagonal() == squares[i].rank() {
                    continue;
                }

                if squares[i].rank().flip_diagonal() > squares[i].file() {
                    for square in &mut squares[i..] {
                        *square = square.flip_diagonal();
                    }
                }

                break;
            }

            if self.num_unique_pieces > 2 {
                let adjust1 = if squares[1] > squares[0] { 1 } else { 0 };
                let adjust2 = if squares[2] > squares[0] { 1 } else { 0 }
                    + if squares[2] > squares[1] { 1 } else { 0 };

                if offdiag(squares[0]) {
                    TRIANGLE[usize::from(squares[0])] * 63 * 62
                        + (u64::from(squares[1]) - adjust1) * 62
                        + (u64::from(squares[2]) - adjust2)
                } else if offdiag(squares[1]) {
                    6 * 63 * 62
                        + squares[0].rank() as u64 * 28 * 62
                        + LOWER[usize::from(squares[1])] * 62
                        + u64::from(squares[2])
                        - adjust2
                } else if offdiag(squares[2]) {
                    6 * 63 * 62
                        + 4 * 28 * 62
                        + squares[0].rank() as u64 * 7 * 28
                        + (squares[1].rank() as u64 - adjust1) * 28
                        + LOWER[usize::from(squares[2])]
                } else {
                    6 * 63 * 62
                        + 4 * 28 * 62
                        + 4 * 7 * 28
                        + squares[0].rank() as u64 * 7 * 6
                        + (squares[1].rank() as u64 - adjust1) * 6
                        + (squares[2].rank() as u64 - adjust2)
                }
            } else if self.num_unique_pieces == 2 {
                if false {
                    let adjust = if squares[1] > squares[0] { 1 } else { 0 };

                    if offdiag(squares[0]) {
                        TRIANGLE[usize::from(squares[0])] * 63 + (u64::from(squares[1]) - adjust)
                    } else if offdiag(squares[1]) {
                        6 * 63 + squares[0].rank() as u64 * 28 + LOWER[usize::from(squares[1])]
                    } else {
                        6 * 63
                            + 4 * 28
                            + squares[0].rank() as u64 * 7
                            + (squares[1].rank() as u64 - adjust)
                    }
                } else {
                    KK_IDX[TRIANGLE[usize::from(squares[0])] as usize][usize::from(squares[1])]
                }
            } else if self.min_like_man == 2 {
                if TRIANGLE[usize::from(squares[0])] > TRIANGLE[usize::from(squares[1])] {
                    squares.swap(0, 1);
                }

                if squares[0].file() >= File::E {
                    for square in &mut squares {
                        *square = square.flip_horizontal();
                    }
                }

                if squares[0].rank() >= Rank::Fifth {
                    for square in &mut squares {
                        *square = square.flip_vertical();
                    }
                }

                if squares[0].rank().flip_diagonal() > squares[0].file()
                    || (!offdiag(squares[0])
                        && squares[1].rank().flip_diagonal() > squares[1].file())
                {
                    for square in &mut squares {
                        *square = square.flip_diagonal();
                    }
                }

                if TEST45.contains(squares[1])
                    && TRIANGLE[usize::from(squares[0])] == TRIANGLE[usize::from(squares[1])]
                {
                    squares.swap(0, 1);

                    for square in &mut squares {
                        *square = square.flip_vertical().flip_diagonal();
                    }
                }

                PP_IDX[TRIANGLE[usize::from(squares[0])] as usize][usize::from(squares[1])]
            } else {
                for i in 1..side.lens[0] {
                    if TRIANGLE[usize::from(squares[0])] > TRIANGLE[usize::from(squares[i])] {
                        squares.swap(0, i);
                    }
                }

                if squares[0].file() >= File::E {
                    for square in &mut squares {
                        *square = square.flip_horizontal();
                    }
                }

                if squares[0].rank() >= Rank::Fifth {
                    for square in &mut squares {
                        *square = square.flip_vertical();
                    }
                }

                if squares[0].rank().flip_diagonal() > squares[0].file() {
                    for square in &mut squares {
                        *square = square.flip_diagonal();
                    }
                }

                for i in 1..side.lens[0] {
                    for j in (i + 1)..side.lens[0] {
                        if MULT_TWIST[usize::from(squares[i])] > MULT_TWIST[usize::from(squares[j])]
                        {
                            squares.swap(i, j);
                        }
                    }
                }

                let mut idx =
                    CONSTS.mult_idx[side.lens[0] - 1][TRIANGLE[usize::from(squares[0])] as usize];
                for i in 1..side.lens[0] {
                    idx += binomial(MULT_TWIST[usize::from(squares[i])], i as u64);
                }

                idx
            }
        };

        idx *= side.factors[0];

        // Encode remaining pawns.
        let mut remaining_pawns =
            material.by_color.white.has_pawns() && material.by_color.black.has_pawns();
        let mut next = 1;
        let mut group_sq = side.lens[0];
        for lens in side.lens.iter().cloned().skip(1) {
            let (prev_squares, group_squares) = squares.split_at_mut(group_sq);
            let group_squares = &mut group_squares[..lens];
            group_squares.sort_unstable();

            let mut n = 0;

            for (i, &group_square) in group_squares.iter().enumerate().take(lens) {
                let adjust = prev_squares[..group_sq]
                    .iter()
                    .filter(|sq| group_square > **sq)
                    .count() as u64;
                n += binomial(
                    u64::from(group_square) - adjust - if remaining_pawns { 8 } else { 0 },
                    i as u64 + 1,
                );
            }

            remaining_pawns = false;
            idx += n * side.factors[next];
            group_sq += side.lens[next];
            next += 1;
        }
        Ok(idx as usize) // u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{fen::Fen, CastlingMode, Chess};

    #[test]
    fn text_encode_function_against_syzygy_value() {
        let material = Material::from_str("KBNvK").unwrap();
        let table = Table::new(&material);
        let chess: Chess = Fen::from_ascii(b"8/8/8/8/8/8/8/KNBk4 w - - 0 1")
            .unwrap()
            .into_position(CastlingMode::Chess960)
            .unwrap();
        let idx = table.encode(&chess);
        assert_eq!(idx, 484157);
    }
}
