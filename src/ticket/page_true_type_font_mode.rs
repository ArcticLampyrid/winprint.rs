use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedTrueTypeFontMode,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageTrueTypeFontMode",
        NS_PSK,
        Some("psk")
    )],
    PageTrueTypeFontMode,
    PredefinedTrueTypeFontMode
);
