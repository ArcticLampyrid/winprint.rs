use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedDuplexType,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "JobDuplexAllDocumentsContiguously",
        NS_PSK,
        Some("psk")
    )],
    JobDuplex,
    PredefinedDuplexType
);
