use super::{document::NS_PSK, PredefinedName};
use std::str::FromStr;
use strum::EnumString;
use xml::name::OwnedName;

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[allow(missing_docs)]
/// Represents a predefined page media type.
pub enum PredefinedPageMediaType {
    Archival,
    AutoSelect,
    BackPrintFilm,
    Bond,
    CardStock,
    // The Print Schema spells this keyword as `psk:Continous`.
    Continous,
    EnvelopePlain,
    EnvelopeWindow,
    Fabric,
    HighResolution,
    Label,
    MultiLayerForm,
    MultiPartForm,
    None,
    Photographic,
    PhotographicFilm,
    PhotographicGlossy,
    PhotographicHighGloss,
    PhotographicMatte,
    PhotographicSatin,
    PhotographicSemiGloss,
    Plain,
    Screen,
    ScreenPaged,
    Stationery,
    TabStockFull,
    TabStockPreCut,
    Transparency,
    TShirtTransfer,
}

impl PredefinedName for PredefinedPageMediaType {
    /// Get the predefined page media type from the given name.
    fn from_name(name: &OwnedName) -> Option<Self> {
        if name.namespace_ref() == Some(NS_PSK) {
            Self::from_str(name.local_name.as_str()).ok()
        } else {
            None
        }
    }
}
