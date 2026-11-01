use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageColorManagement,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageColorManagement",
        NS_PSK,
        Some("psk")
    )],
    PageColorManagement,
    PredefinedPageColorManagement
);

#[cfg(test)]
mod tests {
    use crate::ticket::{
        test_helpers::{assert_converted, capabilities_with_features},
        FeatureOptionPackWithPredefined, PredefinedPageColorManagement,
    };

    #[test]
    fn lists_and_converts_every_predefined_option() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:PageColorManagement">
        <psf:Option name="psk:None" />
        <psf:Option name="psk:Device" />
        <psf:Option name="psk:Driver" />
        <psf:Option name="psk:System" />
        <psf:Option name="ns0000:Vendor" xmlns:ns0000="http://example.com/vendor" />
    </psf:Feature>"#,
        );
        let options = capabilities.page_color_managements().collect::<Vec<_>>();
        assert_eq!(
            options
                .iter()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            [
                Some(PredefinedPageColorManagement::None),
                Some(PredefinedPageColorManagement::Device),
                Some(PredefinedPageColorManagement::Driver),
                Some(PredefinedPageColorManagement::System),
                None
            ]
        );
        options.into_iter().for_each(assert_converted);
    }
}
