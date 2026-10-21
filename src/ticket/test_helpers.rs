use super::{
    document::{
        reader::ParsableXmlDocument, OwnedName, PrintCapabilitiesDocument, PrintFeature,
        PrintFeatureOption, PrintTicketDocument,
    },
    print_capabilities::{features_at_path, name_matches},
    FeatureOptionPack, PrintCapabilities, PrintTicket, PrintTicketBuilder,
};
use crate::printer::PrinterDevice;

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

/// Merge the given deltas, in order, into the driver default print ticket of the device.
pub fn merge_all(
    device: &PrinterDevice,
    deltas: impl IntoIterator<Item = PrintTicket>,
) -> PrintTicketDocument {
    let mut builder = PrintTicketBuilder::new(device).unwrap();
    deltas
        .into_iter()
        .for_each(|delta| builder.merge(delta).unwrap());
    PrintTicketDocument::parse_from_bytes(builder.build().unwrap().get_xml()).unwrap()
}

/// Get the only option that the ticket selects for the given feature path.
pub fn selected_option<'a>(
    document: &'a PrintTicketDocument,
    feature_path: &[OwnedName],
) -> &'a PrintFeatureOption {
    let features = features_at_path(&document.features, feature_path);
    assert_eq!(features.len(), 1, "feature {feature_path:?}");
    assert_eq!(features[0].options.len(), 1, "options of {feature_path:?}");
    &features[0].options[0]
}

/// Merge the pack into the driver default print ticket, and assert that the merged
/// ticket selects the option of the pack.
pub fn assert_merged(device: &PrinterDevice, pack: impl FeatureOptionPack) {
    let feature_path = pack.option_feature_path().to_vec();
    let expected = pack.option().name.clone().expect("named option");
    let document = merge_all(device, [pack.into()]);
    let actual = selected_option(&document, &feature_path)
        .name
        .as_ref()
        .expect("named option");
    assert!(
        name_matches(actual, &expected),
        "expected {expected:?}, got {actual:?}"
    );
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
