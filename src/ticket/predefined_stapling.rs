use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined stapling.
pub enum PredefinedStapling {
    None,
    SaddleStitch,
    StapleBottomLeft,
    StapleBottomRight,
    StapleDualBottom,
    StapleDualLeft,
    StapleDualRight,
    StapleDualTop,
    StapleTopLeft,
    StapleTopRight,
}

impl PredefinedName for PredefinedStapling {
    /// Get the predefined stapling from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
