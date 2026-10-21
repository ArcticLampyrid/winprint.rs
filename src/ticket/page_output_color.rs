use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageOutputColor,
};

define_feature_option_pack!(
    vec![OwnedName::qualified("PageOutputColor", NS_PSK, Some("psk"))],
    PageOutputColor,
    PredefinedPageOutputColor
);
