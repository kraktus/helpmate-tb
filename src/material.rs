// This file is part of the shakmaty-syzygy library.
// Copyright (C) 2017-2021 Niklas Fiekas <niklas.fiekas@backscattering.de>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use std::{
    cmp::{Ord, Ordering, PartialOrd},
    fmt,
};

use serde::Deserialize;
use serde::Deserializer;
use shakmaty::{Board, ByColor, ByRole, Color, Piece, Role};

use crate::Pieces;

use serde;
use serde::de;

#[derive(Clone, Eq, PartialEq, Hash)]
pub(crate) struct MaterialSide {
    by_role: ByRole<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Copy)]
enum CanMate {
    Yes,
    No,
    NeedHelp,
}

impl CanMate {
    fn is_mate_possible(self, other_side: CanMate) -> bool {
        match self {
            Self::Yes => true,
            Self::No => other_side == Self::Yes,
            Self::NeedHelp => other_side != Self::No,
        }
    }
}

impl MaterialSide {
    fn empty() -> MaterialSide {
        MaterialSide {
            by_role: ByRole::default(),
        }
    }

    fn from_str_part(s: &str) -> Result<MaterialSide, ()> {
        let mut side = MaterialSide::empty();
        for ch in s.as_bytes() {
            let role = Role::from_char(char::from(*ch)).ok_or(())?;
            *side.by_role.get_mut(role) += 1;
        }
        Ok(side)
    }

    pub(crate) fn count(&self) -> usize {
        self.by_role.iter().map(|c| usize::from(*c)).sum()
    }

    pub(crate) fn has_pawns(&self) -> bool {
        self.by_role.pawn > 0
    }

    fn unique_roles(&self) -> u8 {
        self.by_role.iter().filter(|c| **c == 1).sum()
    }

    /// All `MaterialSide` configuration than can be possible from this setup using legal moves
    pub fn descendants(&self) -> Vec<MaterialSide> {
        let mut descendants: Vec<MaterialSide> = Vec::with_capacity(6); // arbitrary
                                                                        // a pawn can be promoted
        if self.has_pawns() {
            for role in [Role::Bishop, Role::Knight, Role::Rook, Role::Queen] {
                let mut descendant = self.clone();
                descendant.by_role.pawn -= 1;
                *descendant.by_role.get_mut(role) += 1;
                descendants.push(descendant)
            }
        }
        // all pieces but king can be taken
        for role in [
            Role::Pawn,
            Role::Bishop,
            Role::Knight,
            Role::Rook,
            Role::Queen,
        ] {
            if *self.by_role.get(role) > 0 {
                let mut descendant = self.clone();
                *descendant.by_role.get_mut(role) -= 1;
                descendants.push(descendant)
            }
        }

        descendants
    }

    /// Can this side mate the other one with this material config?
    /// Not taking into accounts bishops on the same color issue
    fn can_mate(&self) -> CanMate {
        if self.count() > 2 || self.by_role.rook > 0 || self.by_role.queen > 0 || self.has_pawns() {
            CanMate::Yes
        } else if self.count() == 2 {
            // should have a knight or bishop only
            CanMate::NeedHelp
        } else {
            // only king
            assert!(self.count() == 1);
            assert!(self.by_role.king == 1);
            CanMate::No
        }
    }
}

impl std::ops::Deref for MaterialSide {
    type Target = ByRole<u8>;

    fn deref(&self) -> &Self::Target {
        &self.by_role
    }
}

impl Ord for MaterialSide {
    fn cmp(&self, other: &MaterialSide) -> Ordering {
        self.count()
            .cmp(&other.count())
            .then(self.by_role.king.cmp(&other.by_role.king))
            .then(self.by_role.queen.cmp(&other.by_role.queen))
            .then(self.by_role.rook.cmp(&other.by_role.rook))
            .then(self.by_role.bishop.cmp(&other.by_role.bishop))
            .then(self.by_role.knight.cmp(&other.by_role.knight))
            .then(self.by_role.pawn.cmp(&other.by_role.pawn))
    }
}

impl PartialOrd for MaterialSide {
    fn partial_cmp(&self, other: &MaterialSide) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for MaterialSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(
            for (role, count) in self.by_role.as_ref().zip_role().into_iter().rev() {
                f.write_str(&role.upper_char().to_string().repeat(usize::from(*count)))?;
            },
        )
    }
}

impl fmt::Debug for MaterialSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.count() > 0 {
            <Self as fmt::Display>::fmt(self, f)
        } else {
            f.write_str("-")
        }
    }
}

/// A material key.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Material {
    pub(crate) by_color: ByColor<MaterialSide>,
}

impl Material {
    fn empty() -> Material {
        Material {
            by_color: ByColor::new_with(|_| MaterialSide::empty()),
        }
    }

    /// Get the material configuration for a [`Board`].
    pub fn from_board(board: &Board) -> Material {
        Material {
            by_color: ByColor::new_with(|color| MaterialSide {
                by_role: board.material_side(color),
            }),
        }
    }

    pub(crate) fn from_iter<I>(iter: I) -> Material
    where
        I: IntoIterator<Item = Piece>,
    {
        let mut material = Material::empty();
        for piece in iter {
            *material
                .by_color
                .get_mut(piece.color)
                .by_role
                .get_mut(piece.role) += 1;
        }
        material
    }

    pub(crate) fn from_str(s: &str) -> Result<Material, ()> {
        if s.len() > 64 + 1 {
            return Err(());
        }

        let (white, black) = s.split_once('v').ok_or(())?;
        Ok(Material {
            by_color: ByColor {
                white: MaterialSide::from_str_part(white)?,
                black: MaterialSide::from_str_part(black)?,
            },
        })
    }

    pub(crate) fn count(&self) -> usize {
        self.by_color.iter().map(|side| side.count()).sum()
    }

    pub(crate) fn is_symmetric(&self) -> bool {
        self.by_color.white == self.by_color.black
    }

    pub(crate) fn has_pawns(&self) -> bool {
        self.by_color.iter().any(|side| side.has_pawns())
    }

    pub(crate) fn unique_pieces(&self) -> u8 {
        self.by_color.iter().map(|side| side.unique_roles()).sum()
    }

    pub(crate) fn min_like_man(&self) -> u8 {
        self.by_color
            .iter()
            .flat_map(|side| side.by_role.iter())
            .copied()
            .filter(|c| 2 <= *c)
            .min()
            .unwrap_or(0)
    }

    pub(crate) fn into_flipped(self) -> Material {
        Material {
            by_color: self.by_color.into_flipped(),
        }
    }

    /// For any color
    pub(crate) fn is_mate_possible(&self) -> bool {
        // order is arbitrary
        let (white, black) = (
            self.by_color.white.can_mate(),
            self.by_color.black.can_mate(),
        );
        white.is_mate_possible(black)
    }

    // pub(crate) fn into_normalized(self) -> Material {
    //     Material {
    //         by_color: self.by_color.into_normalized(),
    //     }
    // }

    pub(crate) fn by_piece(&self, piece: Piece) -> u8 {
        *self.by_color.get(piece.color).get(piece.role)
    }

    pub(crate) fn pieces(&self) -> Pieces {
        self.pieces_with_white_king(true)
    }

    fn pieces_with_white_king(&self, with_white_king: bool) -> Pieces {
        let mut pieces = Pieces::new();
        for color in Color::ALL {
            for role in Role::ALL {
                let piece = Piece { color, role };
                if !(!with_white_king && piece == Color::White.king()) {
                    for _ in 0..self.by_piece(piece) {
                        pieces.push(piece)
                    }
                }
            }
        }
        pieces
    }

    pub(crate) fn pieces_without_white_king(&self) -> Pieces {
        self.pieces_with_white_king(false)
    }
}

impl fmt::Display for Material {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.by_color.white, self.by_color.black)
    }
}

struct MaterialVisitor;

impl<'de> de::Visitor<'de> for MaterialVisitor {
    type Value = Material;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string containing json data")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Material::from_str(v).unwrap())
    }
}

impl<'de> Deserialize<'de> for Material {
    fn deserialize<D>(deserializer: D) -> Result<Material, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MaterialVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::Color::{Black, White};
    use std::collections::HashSet;

    #[test]
    fn test_pieces_without_white_king_from_material() {
        let mat = Material::from_str("KBNvKRQ").unwrap();
        let pieces: Pieces = (&[
            White.knight(),
            White.bishop(),
            Black.rook(),
            Black.queen(),
            Black.king(),
        ] as &[_])
            .try_into()
            .unwrap();
        assert_eq!(mat.pieces_without_white_king(), pieces)
    }

    #[test]
    fn test_material_side_descendants() {
        // (ancester, descendants)
        for test_config in [
            ("KN", vec!["K"]),
            ("KP", vec!["K", "KN", "KB", "KR", "KQ"]),
            ("KPP", vec!["KP", "KPN", "KPB", "KPR", "KPQ"]),
            ("KRR", vec!["KR"]),
            ("K", vec![]),
        ] {
            let mat = MaterialSide::from_str_part(test_config.0).unwrap();
            assert_eq!(
                HashSet::from_iter(mat.descendants().into_iter()),
                HashSet::<MaterialSide>::from_iter(
                    test_config
                        .1
                        .iter()
                        .map(|s| MaterialSide::from_str_part(s).unwrap())
                )
            );
        }
    }

    #[test]
    fn test_material_side_can_mate() {
        for test_config in [
            ("KN", CanMate::NeedHelp),
            ("KB", CanMate::NeedHelp),
            ("KBB", CanMate::Yes),
            ("KNN", CanMate::Yes),
            ("KP", CanMate::Yes),
            ("KPP", CanMate::Yes),
            ("KRR", CanMate::Yes),
            ("K", CanMate::No),
        ] {
            let mat = MaterialSide::from_str_part(test_config.0).unwrap();
            assert_eq!(mat.can_mate(), test_config.1);
        }
    }

    #[test]
    fn test_is_mate_possible() {
        for test_config in [
            ("KBNvKRQ", true),
            ("KNvKB", true),
            ("KBvK", false),
            ("KvKB", false),
            ("KNvK", false),
            ("KvK", false),
            ("KPvK", true),
            ("KPvKP", true),
            ("KRvP", true),
            ("KQvP", true),
        ] {
            let mat = Material::from_str(test_config.0).unwrap();
            assert_eq!(mat.is_mate_possible(), test_config.1);
        }
    }
}
