use super::document::NS_PSK;
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined page orientation.
pub enum PredefinedPageOrientation {
    Portrait,
    Landscape,
    ReversePortrait,
    ReverseLandscape,
}

impl PredefinedPageOrientation {
    /// Get predefined media name from the given name.
    pub fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
