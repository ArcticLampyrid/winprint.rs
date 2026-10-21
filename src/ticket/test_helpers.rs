use super::{
    document::{
        reader::ParsableXmlDocument, PrintCapabilitiesDocument, PrintFeature, PrintTicketDocument,
    },
    FeatureOptionPack, PrintCapabilities, PrintTicket,
};

/// Parse the print capabilities from the given XML document.
pub fn capabilities_from_xml(xml: impl AsRef<[u8]>) -> PrintCapabilities {
    PrintCapabilities {
        document: PrintCapabilitiesDocument::parse_from_bytes(xml).unwrap(),
    }
}

/// Parse print capabilities that define the given features.
pub fn capabilities_with_features(features: &str) -> PrintCapabilities {
    capabilities_from_xml(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<psf:PrintCapabilities
xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework"
xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords" version="1">
{features}
</psf:PrintCapabilities>"#
    ))
}

/// Assert that the ticket converted from the pack carries exactly the option of the
/// pack, nested along its feature path under ancestors that carry no option.
pub fn assert_converted(pack: impl FeatureOptionPack) {
    let feature_path = pack.option_feature_path().to_vec();
    let expected = pack.option().clone();
    let document =
        PrintTicketDocument::parse_from_bytes(PrintTicket::from(pack).get_xml()).unwrap();
    let leaf = feature_path
        .iter()
        .fold(None::<&PrintFeature>, |parent, name| {
            let siblings = match parent {
                None => document.features.as_slice(),
                Some(parent) => {
                    assert!(
                        parent.options.is_empty(),
                        "{:?} carries an option",
                        parent.name
                    );
                    parent.features.as_slice()
                }
            };
            assert_eq!(siblings.len(), 1, "features named {name:?}");
            assert_eq!(&siblings[0].name, name);
            Some(&siblings[0])
        })
        .expect("non-empty feature path");
    assert_eq!(leaf.options, vec![expected]);
}
