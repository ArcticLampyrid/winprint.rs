use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
/// Represents a predefined page destination color profile: where the profile of the device comes from.
pub enum PredefinedPageDestinationColorProfile {
    /// The application chose the destination profile and has converted the colors with it.
    Application,
    /// The driver's configuration chooses the destination profile.
    DriverConfiguration,
}

impl PredefinedName for PredefinedPageDestinationColorProfile {
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
