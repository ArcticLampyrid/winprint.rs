use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined page borderless option.
pub enum PredefinedPageBorderless {
    Borderless,
    None,
}

impl PredefinedName for PredefinedPageBorderless {
    /// Get the predefined page borderless option from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
