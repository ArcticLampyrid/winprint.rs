use xml::name::OwnedName;

use super::{
    document::{
        ParameterInit, PrintFeature, PrintFeatureOption, PrintTicketDocument, WithProperties,
        NS_PSK,
    },
    PredefinedPageOrientation, PrintTicket,
};

#[derive(Clone, Debug)]
/// Represents a page orientation.
pub struct PageOrientation {
    /// The DOM of the option for page orientation.
    pub option: PrintFeatureOption,
    /// The parameters that is referenced by the option.
    pub parameters: Vec<ParameterInit>,
}

impl PageOrientation {
    /// Create a new [`PageOrientation`] from the given DOM.
    pub fn new(option: PrintFeatureOption, parameters: Vec<ParameterInit>) -> Self {
        Self { option, parameters }
    }

    /// Get display name of the page orientation.
    pub fn display_name(&self) -> Option<&str> {
        self.option
            .get_property("DisplayName", Some(NS_PSK))
            .and_then(|x| x.value.as_ref())
            .and_then(|x| x.string())
    }

    /// Get the predefined name of the page orientation.
    /// If the page orientation is not predefined, `None` is returned.
    pub fn as_predefined_name(&self) -> Option<PredefinedPageOrientation> {
        self.option
            .name
            .as_ref()
            .and_then(PredefinedPageOrientation::from_name)
    }
}

impl From<PageOrientation> for PrintTicket {
    fn from(value: PageOrientation) -> Self {
        PrintTicketDocument {
            properties: vec![],
            parameter_inits: value.parameters,
            features: vec![PrintFeature {
                name: OwnedName::qualified("PageOrientation", NS_PSK, Some("psk")),
                properties: vec![],
                options: vec![value.option],
                features: vec![],
            }],
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        test_utils::null_device,
        ticket::{PredefinedPageOrientation, PrintCapabilities, PrintTicketBuilder},
    };
    #[test]
    fn use_landscape() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let option = capabilities
            .page_orientation()
            .find(|x| x.as_predefined_name() == Some(PredefinedPageOrientation::Landscape))
            .unwrap();
        println!("Using {:?}", option.display_name());
        let mut builder = PrintTicketBuilder::new(&device).unwrap();
        builder.merge(option).unwrap();
    }
}
