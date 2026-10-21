use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageOutputQuality,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageOutputQuality",
        NS_PSK,
        Some("psk")
    )],
    PageOutputQuality,
    PredefinedPageOutputQuality
);
