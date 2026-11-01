use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageDestinationColorProfile,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageDestinationColorProfile",
        NS_PSK,
        Some("psk")
    )],
    PageDestinationColorProfile,
    PredefinedPageDestinationColorProfile
);

#[cfg(test)]
mod tests {
    use crate::ticket::{
        test_helpers::{assert_converted, capabilities_with_features},
        FeatureOptionPackWithPredefined, PredefinedPageDestinationColorProfile,
    };

    #[test]
    fn lists_and_converts_every_predefined_option() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:PageDestinationColorProfile">
        <psf:Option name="psk:Application" />
        <psf:Option name="psk:DriverConfiguration" />
        <psf:Option name="ns0000:Vendor" xmlns:ns0000="http://example.com/vendor" />
    </psf:Feature>"#,
        );
        let options = capabilities
            .page_destination_color_profiles()
            .collect::<Vec<_>>();
        assert_eq!(
            options
                .iter()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            [
                Some(PredefinedPageDestinationColorProfile::Application),
                Some(PredefinedPageDestinationColorProfile::DriverConfiguration),
                None
            ]
        );
        options.into_iter().for_each(assert_converted);
    }
}
