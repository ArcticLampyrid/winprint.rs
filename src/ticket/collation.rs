use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedCollation,
};

define_feature_option_pack!(
    vec![OwnedName::qualified("DocumentCollate", NS_PSK, Some("psk"))],
    Collation,
    PredefinedCollation
);

#[cfg(test)]
mod tests {
    use crate::{
        test_utils::null_device,
        ticket::{
            test_helpers::assert_merged, FeatureOptionPackWithPredefined, PredefinedCollation,
            PrintCapabilities,
        },
    };

    #[test]
    fn use_collated() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let collated = capabilities
            .collations()
            .find(|x| x.as_predefined_name() == Some(PredefinedCollation::Collated))
            .unwrap();
        assert_merged(&device, collated);
    }
}
