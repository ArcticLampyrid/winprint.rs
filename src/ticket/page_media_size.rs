use super::{
    document::{
        ParameterInit, PrintFeature, PrintFeatureOption, PrintTicketDocument, WithScoredProperties,
        NS_PSK,
    },
    MediaSizeTuple, PredefinedMediaName, PrintTicket,
};
use xml::name::OwnedName;

#[derive(Clone, Debug)]
pub struct PageMediaSize {
    pub option: PrintFeatureOption,
    pub parameters: Vec<ParameterInit>,
}

impl PageMediaSize {
    pub fn new(option: PrintFeatureOption, parameters: Vec<ParameterInit>) -> Self {
        Self { option, parameters }
    }

    pub fn size(&self) -> MediaSizeTuple {
        let width = self
            .option
            .get_scored_property("MediaSizeWidth", Some(NS_PSK))
            .and_then(|x| x.value_with(&self.parameters))
            .and_then(|x| x.integer())
            .unwrap_or_default();
        let height = self
            .option
            .get_scored_property("MediaSizeHeight", Some(NS_PSK))
            .and_then(|x| x.value_with(&self.parameters))
            .and_then(|x| x.integer())
            .unwrap_or_default();
        MediaSizeTuple::micron(width as u32, height as u32)
    }

    pub fn as_predefined_name(&self) -> Option<PredefinedMediaName> {
        self.option
            .name
            .as_ref()
            .and_then(PredefinedMediaName::from_name)
    }
}

impl From<PageMediaSize> for PrintTicket {
    fn from(value: PageMediaSize) -> Self {
        PrintTicketDocument {
            properties: vec![],
            parameter_inits: value.parameters,
            features: vec![PrintFeature {
                name: OwnedName::qualified("PageMediaSize", NS_PSK, Some("psk")),
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
        tests::get_test_device,
        ticket::{PredefinedMediaName, PrintCapabilities, PrintTicketBuilder},
    };

    #[test]
    fn use_page_media_size() {
        let device = get_test_device();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        for media in capabilities.page_media_size() {
            let mut builder = PrintTicketBuilder::new(&device).unwrap();
            builder.merge(media).unwrap();
        }
    }

    #[test]
    fn get_size() {
        let device = get_test_device();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        for media in capabilities.page_media_size() {
            let size = media.size();
            assert!(
                size.width_in_micron() | size.height_in_micron() > 0,
                "Size is zero for {:#?}",
                media
            );
        }
    }

    #[test]
    fn a4_should_be_found() {
        let device = get_test_device();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        capabilities
            .page_media_size()
            .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
            .unwrap();
    }
}
