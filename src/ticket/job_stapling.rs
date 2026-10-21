use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedStapling,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "JobStapleAllDocuments",
        NS_PSK,
        Some("psk")
    )],
    JobStapling,
    PredefinedStapling
);
