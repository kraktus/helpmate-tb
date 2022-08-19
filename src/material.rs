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

use std::ops::Deref;

use itertools::Itertools as _;
use serde::Deserialize;
use serde::Deserializer;
use shakmaty::{Board, ByColor, ByRole, Color, Piece, Role};

use crate::Pieces;
use std::iter;

use serde::de;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct MaterialSide {
    by_role: ByRole<u8>,
}

impl From<ByRole<u8>> for MaterialSide {
    fn from(by_role: ByRole<u8>) -> Self {
        Self { by_role }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Copy)]
enum CanMate {
    Yes,
    No,
    NeedHelp,
}

impl CanMate {
    fn is_mate_possible(self, other_side: Self) -> bool {
        match self {
            Self::Yes => true,
            Self::No => other_side == Self::Yes,
            Self::NeedHelp => other_side != Self::No,
        }
    }
}

impl MaterialSide {
    fn empty() -> Self {
        Self {
            by_role: ByRole::default(),
        }
    }

    fn from_str_part(s: &str) -> Result<Self, ()> {
        let mut side = Self::empty();
        for ch in s.as_bytes() {
            let role = Role::from_char(char::from(*ch)).ok_or(())?;
            *side.by_role.get_mut(role) += 1;
        }
        Ok(side)
    }

    pub fn count(&self) -> usize {
        self.by_role.iter().map(|c| usize::from(*c)).sum()
    }

    pub fn has_pawns(&self) -> bool {
        self.by_role.pawn > 0
    }

    fn unique_roles(&self) -> u8 {
        self.by_role.iter().filter(|c| **c == 1).sum()
    }

    /// All `MaterialSide` configuration than can be possible from this setup using legal moves
    pub fn descendants(&self) -> Vec<Self> {
        let mut descendants: Vec<Self> = Vec::with_capacity(6); // arbitrary
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

impl Deref for MaterialSide {
    type Target = ByRole<u8>;

    fn deref(&self) -> &Self::Target {
        &self.by_role
    }
}

impl Ord for MaterialSide {
    fn cmp(&self, other: &Self) -> Ordering {
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
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

/// Wrapper to ensure `Material` is always normalised
/// There should be no way to mutate it, and only one way to create it:
/// `From<ByColor<MaterialSide>>`
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ByColorNormalisedMaterialSide(ByColor<MaterialSide>);

impl From<ByColor<MaterialSide>> for ByColorNormalisedMaterialSide {
    fn from(by_color: ByColor<MaterialSide>) -> Self {
        Self(by_color.into_normalized())
    }
}

impl Deref for ByColorNormalisedMaterialSide {
    type Target = ByColor<MaterialSide>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A material key.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Material {
    pub by_color: ByColorNormalisedMaterialSide,
}

impl Ord for Material {
    fn cmp(&self, other: &Self) -> Ordering {
        self.count()
            .cmp(&other.count())
            .then(self.by_color.white.cmp(&other.by_color.white))
            .then(self.by_color.black.cmp(&other.by_color.black))
    }
}

impl PartialOrd for Material {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Material {
    /// Get the material configuration for a [`Board`].
    pub fn from_board(board: &Board) -> Self {
        Self {
            by_color: ByColor::new_with(|color| MaterialSide {
                by_role: board.material_side(color),
            })
            .into(),
        }
    }

    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Piece>,
    {
        let mut by_color = ByColor::new_with(|_| MaterialSide::empty());
        for piece in iter {
            *by_color.get_mut(piece.color).by_role.get_mut(piece.role) += 1;
        }
        Self {
            by_color: by_color.into(),
        }
    }

    pub fn from_str(s: &str) -> Result<Self, ()> {
        if s.len() > 64 + 1 {
            return Err(());
        }

        let (white, black) = s.split_once('v').ok_or(())?;
        Ok(Self {
            by_color: ByColor {
                white: MaterialSide::from_str_part(white)?,
                black: MaterialSide::from_str_part(black)?,
            }
            .into(),
        })
    }

    pub fn count(&self) -> usize {
        self.by_color.iter().map(|side| side.count()).sum()
    }

    pub fn is_symmetric(&self) -> bool {
        self.by_color.white == self.by_color.black
    }

    pub fn has_pawns(&self) -> bool {
        self.by_color.iter().any(|side| side.has_pawns())
    }

    pub fn unique_pieces(&self) -> u8 {
        self.by_color.iter().map(|side| side.unique_roles()).sum()
    }

    pub fn min_like_man(&self) -> u8 {
        self.by_color
            .iter()
            .flat_map(|side| side.by_role.iter())
            .copied()
            .filter(|c| 2 <= *c)
            .min()
            .unwrap_or(0)
    }

    /// For any color
    pub fn is_mate_possible(&self) -> bool {
        // order is arbitrary
        let (white, black) = (
            self.by_color.white.can_mate(),
            self.by_color.black.can_mate(),
        );
        white.is_mate_possible(black)
    }

    /// For any color
    fn descendants(&self) -> impl Iterator<Item = Self> + '_ {
        self.by_color
            .iter()
            .circular_tuple_windows()
            .flat_map(|(mat_1, mat_2)| {
                mat_1
                    .descendants()
                    .into_iter()
                    .map(|mat_1_descendant| Self {
                        by_color: ByColor {
                            white: mat_1_descendant,
                            black: mat_2.clone(),
                        }
                        .into(),
                    })
            })
    }

    /// For any color, depth 1 descendants not trivially drawn
    /// If looking for all descendants, incluring indirect ones, use `Material::descendants_not_draw_recursive` instead
    pub fn descendants_not_draw(&self) -> impl Iterator<Item = Self> + '_ {
        self.descendants().filter(Self::is_mate_possible)
    }

    /// Vec containing all unique material configurations, not containing the root material
    /// Sorted by positions with fewer pieces first
    pub fn descendants_not_draw_recursive(&self) -> Vec<Self> {
        let mut descendants_recursive: Vec<Self> = self.descendants_not_draw_recursive_internal();
        println!("{:?}", descendants_recursive);
        descendants_recursive.sort();
        descendants_recursive.dedup();
        descendants_recursive
    }

    #[inline]
    fn descendants_not_draw_recursive_internal(&self) -> Vec<Self> {
        self.descendants_not_draw()
            .flat_map(|x| {
                iter::once(x.clone()).chain(x.descendants_not_draw_recursive_internal().into_iter())
            })
            .collect()
    }

    pub fn by_piece(&self, piece: Piece) -> u8 {
        *self.by_color.get(piece.color).get(piece.role)
    }

    fn pieces_with_white_king(&self, with_white_king: bool) -> Pieces {
        let mut pieces = Pieces::new();
        for color in Color::ALL {
            for role in Role::ALL {
                let piece = Piece { color, role };
                if with_white_king || !(piece == Color::White.king()) {
                    for _ in 0..self.by_piece(piece) {
                        pieces.push(piece)
                    }
                }
            }
        }
        pieces
    }

    pub fn pieces_without_white_king(&self) -> Pieces {
        self.pieces_with_white_king(false)
    }
}

impl fmt::Debug for Material {
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
        let mat = Material::from_str("KRQvKBN").unwrap();
        let pieces: Pieces = (&[
            White.rook(),
            White.queen(),
            Black.knight(),
            Black.bishop(),
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
            ("KRvKP", true),
            ("KQvKP", true),
        ] {
            let mat = Material::from_str(test_config.0).unwrap();
            assert_eq!(mat.is_mate_possible(), test_config.1);
        }
    }

    #[test]
    fn test_material_descendants() {
        for test_config in [
            ("KvK", vec![]),
            ("KBvK", vec!["KvK"]),
            ("KNvK", vec!["KvK"]),
            ("KRvK", vec!["KvK"]),
            ("KQvK", vec!["KvK"]),
            ("KBNvK", vec!["KBvK", "KNvK"]),
            ("KRRvK", vec!["KRvK"]),
            ("KPvK", vec!["KBvK", "KNvK", "KRvK", "KQvK", "KvK"]),
        ] {
            let mat = Material::from_str(test_config.0).unwrap();
            assert_eq!(
                HashSet::from_iter(mat.descendants()),
                HashSet::<Material>::from_iter(
                    test_config.1.iter().map(|s| Material::from_str(s).unwrap())
                )
            );
        }
    }

    #[test]
    fn test_material_descendants_not_draw() {
        for test_config in [
            ("KvK", vec![]),
            ("KBvK", vec![]),
            ("KNvK", vec![]),
            ("KRvK", vec![]),
            ("KQvK", vec![]),
            ("KBNvK", vec![]),
            ("KRRvK", vec!["KRvK"]),
            ("KPvK", vec!["KRvK", "KQvK"]),
            ("KQRvK", vec!["KQvK", "KRvK"]),
            ("KRvQK", vec!["KQvK", "KRvK"]),
        ] {
            let mat = Material::from_str(test_config.0).unwrap();
            assert_eq!(
                HashSet::from_iter(mat.descendants_not_draw()),
                HashSet::<Material>::from_iter(
                    test_config.1.iter().map(|s| Material::from_str(s).unwrap())
                )
            );
        }
    }

    #[test]
    fn test_material_descendants_not_draw_recursive() {
        for test_config in [
            ("KvK", vec![]),
            ("KBvK", vec![]),
            ("KNvK", vec![]),
            ("KRvK", vec![]),
            ("KQvK", vec![]),
            ("KBNvK", vec![]),
            ("KRRvK", vec!["KRvK"]),
            ("KPvK", vec!["KRvK", "KQvK"]),
            ("KQRvK", vec!["KRvK", "KQvK"]),
            ("KRvQK", vec!["KRvK", "KQvK"]),
            ("KRBNvK", vec!["KRvK", "KBNvK", "KRNvK", "KRBvK"]),
        ] {
            let mat = Material::from_str(test_config.0).unwrap();
            println!("{mat:?}",);
            assert_eq!(
                mat.descendants_not_draw_recursive(),
                Vec::from_iter(test_config.1.iter().map(|s| Material::from_str(s).unwrap()))
            );
        }
    }

    #[test]
    fn test_material_buildin_normalisation() {
        for test_config in [
            ("KBNvKRQ", "KRQvKBN"),
            ("KNvKB", "KBvKN"),
            ("KBvK", "KvKB"),
            ("KNvK", "KvKN"),
            ("KPvK", "KvKP"),
            ("KRvKP", "KPvKR"),
            ("KQvKP", "KPvKQ"),
        ] {
            assert_eq!(
                Material::from_str(test_config.0).unwrap(),
                Material::from_str(test_config.1).unwrap()
            );
        }
    }
}
