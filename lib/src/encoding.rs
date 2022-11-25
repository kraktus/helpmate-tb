use crate::Material;
use crate::Pieces;
use arrayvec::ArrayVec;
use retroboard::shakmaty::Piece;

use serde::{de, Deserialize};
use std::collections::HashMap;
use std::error::Error;
use std::{fmt, fs};

#[derive(Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct GroupDataInfo {
    #[serde(deserialize_with = "deserialize_json_string")]
    pub pieces: Pieces,
    pub order: [u8; 2],
}

fn deserialize_json_string<'de, D>(deserializer: D) -> Result<Pieces, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct JsonStringVisitor;

    impl<'de> de::Visitor<'de> for JsonStringVisitor {
        type Value = Pieces;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string containing json data")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // unfortunately we lose some typed information
            // from errors deserializing the json string
            Ok(v.chars().map(|c| Piece::from_char(c).unwrap()).collect())
        }
    }

    // use our visitor to deserialize an `ActualValue`
    deserializer.deserialize_any(JsonStringVisitor)
}

pub type InfoTable = ArrayVec<ArrayVec<GroupDataInfo, 2>, 4>;

pub fn get_info_table(m: &Material) -> Result<InfoTable, Box<dyn Error>> {
    // hackfix to allow calling the info table from multiple paths
    let data =
        fs::read_to_string("lib/encoding.json").or_else(|_| fs::read_to_string("encoding.json"))?;
    let map: HashMap<Material, InfoTable> = serde_json::from_str(&data)?;
    Ok(map
        .get(m)
        .expect("Material configuration not found, double check it is legal (1 king of each color)")
        .clone())
}
