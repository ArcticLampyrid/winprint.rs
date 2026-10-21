use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageMediaType,
};

define_feature_option_pack!(
    vec![OwnedName::qualified("PageMediaType", NS_PSK, Some("psk"))],
    PageMediaType,
    PredefinedPageMediaType
);
