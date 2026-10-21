use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined TrueType font mode.
pub enum PredefinedTrueTypeFontMode {
    Automatic,
    DownloadAsNativeTrueTypeFont,
    DownloadAsOutlineFont,
    DownloadAsRasterFont,
    RenderAsBitmap,
}

impl PredefinedName for PredefinedTrueTypeFontMode {
    /// Get the predefined TrueType font mode from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
