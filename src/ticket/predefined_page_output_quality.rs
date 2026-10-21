use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined page output quality.
pub enum PredefinedPageOutputQuality {
    Automatic,
    Draft,
    Fax,
    High,
    Normal,
    Photographic,
    Text,
}

impl PredefinedName for PredefinedPageOutputQuality {
    /// Get the predefined page output quality from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
