use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
/// Represents a predefined ICM rendering intent: how colors outside the device's gamut are mapped.
pub enum PredefinedPageIcmRenderingIntent {
    /// Keep in-gamut colors exactly, including the white point.
    AbsoluteColorimetric,
    /// Keep in-gamut colors, mapping the white point.
    RelativeColorimetric,
    /// Perceptual: keep the relations between colors.
    Photographs,
    /// Saturation: keep colors vivid.
    BusinessGraphics,
}

impl PredefinedName for PredefinedPageIcmRenderingIntent {
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
