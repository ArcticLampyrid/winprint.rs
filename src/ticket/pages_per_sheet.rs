use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, WithScoredProperties, NS_PSK},
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "JobNUpAllDocumentsContiguously",
        NS_PSK,
        Some("psk")
    )],
    PagesPerSheet
);

impl PagesPerSheet {
    /// Get the number of logical pages printed on a single side of a sheet,
    /// or `0` if the device does not report it.
    pub fn pages_per_sheet(&self) -> u32 {
        self.option
            .get_scored_property("PagesPerSheet", Some(NS_PSK))
            .and_then(|x| x.value_with(&self.parameters))
            .and_then(|x| x.integer())
            .and_then(|x| u32::try_from(x).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        test_utils::null_device,
        ticket::{
            test_helpers::{merge_all, selected_option},
            PagesPerSheet, PrintCapabilities, StaticFeaturePathOptionPack,
        },
    };

    #[test]
    fn use_pages_per_sheet() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        for nup in capabilities.pages_per_sheet() {
            let expected = nup.pages_per_sheet();
            let document = merge_all(&device, [nup.into()]);
            let merged = PagesPerSheet::new(
                selected_option(&document, PagesPerSheet::feature_path()).clone(),
                document.parameter_inits.clone(),
            );
            assert_eq!(merged.pages_per_sheet(), expected);
        }
    }

    #[test]
    fn get_pages_per_sheet() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let counts = capabilities
            .pages_per_sheet()
            .map(|x| x.pages_per_sheet())
            .collect::<Vec<_>>();
        assert_eq!(counts, vec![1, 2, 4, 6, 9, 16]);
    }
}
