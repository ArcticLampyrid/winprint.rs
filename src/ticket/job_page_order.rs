use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPageOrder,
};

define_feature_option_pack!(
    vec![OwnedName::qualified("JobPageOrder", NS_PSK, Some("psk"))],
    JobPageOrder,
    PredefinedPageOrder
);
