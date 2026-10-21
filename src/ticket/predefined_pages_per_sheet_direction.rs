use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined pages per sheet direction.
pub enum PredefinedPagesPerSheetDirection {
    BottomLeft,
    BottomRight,
    LeftBottom,
    LeftTop,
    RightBottom,
    RightTop,
    TopLeft,
    TopRight,
}

impl PredefinedName for PredefinedPagesPerSheetDirection {
    /// Get the predefined pages per sheet direction from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
