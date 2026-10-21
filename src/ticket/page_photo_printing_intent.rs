use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPhotoPrintingIntent,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PagePhotoPrintingIntent",
        NS_PSK,
        Some("psk")
    )],
    PagePhotoPrintingIntent,
    PredefinedPhotoPrintingIntent
);
