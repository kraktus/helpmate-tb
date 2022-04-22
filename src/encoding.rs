use serde_json;

use crate::Material;
use crate::Pieces;
use arrayvec::ArrayVec;
use serde;
use serde::de;
use shakmaty::Piece;
use std::error::Error;
use std::{fmt, fs};

#[derive(Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct GroupDataInfo {
    #[serde(deserialize_with = "deserialize_json_string")]
    pub pieces: Pieces,
    pub order: [u8; 2],
}

fn p(s: &str) -> Pieces {
    s.chars().map(|c| Piece::from_char(c).unwrap()).collect()
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
            Ok(p(v))
        }
    }

    // use our visitor to deserialize an `ActualValue`
    deserializer.deserialize_any(JsonStringVisitor)
}

pub type InfoTable = ArrayVec<ArrayVec<GroupDataInfo, 2>, 4>;

pub fn get_info_table(m: &Material) -> Result<InfoTable, Box<dyn Error>> {
    let input = fs::read_to_string("encoding.json")?;
    // let vec: Vec<String
}
