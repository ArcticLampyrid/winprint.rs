use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedDeviceFontSubstitution,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageDeviceFontSubstitution",
        NS_PSK,
        Some("psk")
    )],
    PageDeviceFontSubstitution,
    PredefinedDeviceFontSubstitution
);
