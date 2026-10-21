use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageBorderless,
};

define_feature_option_pack!(
    vec![OwnedName::qualified("PageBorderless", NS_PSK, Some("psk"))],
    PageBorderless,
    PredefinedPageBorderless
);
