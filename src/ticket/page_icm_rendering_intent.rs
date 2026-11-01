use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageIcmRenderingIntent,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageICMRenderingIntent",
        NS_PSK,
        Some("psk")
    )],
    PageIcmRenderingIntent,
    PredefinedPageIcmRenderingIntent
);

#[cfg(test)]
mod tests {
    use crate::ticket::{
        test_helpers::{assert_converted, capabilities_with_features},
        FeatureOptionPackWithPredefined, PredefinedPageIcmRenderingIntent,
    };

    #[test]
    fn lists_and_converts_every_predefined_option() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:PageICMRenderingIntent">
        <psf:Option name="psk:AbsoluteColorimetric" />
        <psf:Option name="psk:RelativeColorimetric" />
        <psf:Option name="psk:Photographs" />
        <psf:Option name="psk:BusinessGraphics" />
        <psf:Option name="ns0000:Vendor" xmlns:ns0000="http://example.com/vendor" />
    </psf:Feature>"#,
        );
        let options = capabilities
            .page_icm_rendering_intents()
            .collect::<Vec<_>>();
        assert_eq!(
            options
                .iter()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            [
                Some(PredefinedPageIcmRenderingIntent::AbsoluteColorimetric),
                Some(PredefinedPageIcmRenderingIntent::RelativeColorimetric),
                Some(PredefinedPageIcmRenderingIntent::Photographs),
                Some(PredefinedPageIcmRenderingIntent::BusinessGraphics),
                None
            ]
        );
        options.into_iter().for_each(assert_converted);
    }
}
