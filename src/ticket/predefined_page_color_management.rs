use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
/// Represents a predefined page color management: where the colors of the page are managed.
pub enum PredefinedPageColorManagement {
    /// No color management: the application has already converted the colors for the device.
    None,
    /// The device manages the colors.
    Device,
    /// The driver manages the colors.
    Driver,
    /// The system manages the colors.
    System,
}

impl PredefinedName for PredefinedPageColorManagement {
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
