use super::{
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    FeatureOptionPack, FeatureOptionPackWithPredefined, PredefinedInputBin, PrintCapabilities,
};

#[derive(Clone, Debug)]
/// Represents a feature option pack as [`InputBin`].
///
/// The Print Schema defines the input bin as three features that differ in scope,
/// `JobInputBin`, `DocumentInputBin` and `PageInputBin`. An option is taken from the
/// first of them that the device supports, so the pack keeps the path of the feature
/// that its option belongs to.
pub struct InputBin {
    /// The path of the feature that the option belongs to.
    feature_path: &'static [OwnedName],
    /// The option of the feature.
    option: PrintFeatureOption,
    /// The parameters that is used by the option.
    parameters: Vec<ParameterInit>,
}

impl InputBin {
    /// The input bin features, ordered from the widest scope to the narrowest.
    fn feature_paths() -> &'static [Vec<OwnedName>; 3] {
        static PATHS: std::sync::LazyLock<[Vec<OwnedName>; 3]> = std::sync::LazyLock::new(|| {
            ["JobInputBin", "DocumentInputBin", "PageInputBin"]
                .map(|name| vec![OwnedName::qualified(name, NS_PSK, Some("psk"))])
        });
        &PATHS
    }

    /// List the input bins of the first supported input bin feature.
    ///
    /// `JobInputBin` is used when the device supports it, and only otherwise
    /// `DocumentInputBin`, and only otherwise `PageInputBin`.
    pub fn list(capabilities: &PrintCapabilities) -> impl Iterator<Item = Self> + '_ {
        Self::feature_paths()
            .iter()
            .find_map(|feature_path| {
                let mut options = capabilities.options_for_feature(feature_path).peekable();
                options.peek().is_some().then_some((feature_path, options))
            })
            .into_iter()
            .flat_map(move |(feature_path, options)| {
                options.map(move |option| Self {
                    feature_path,
                    option: option.clone(),
                    parameters: capabilities.default_parameters_for_option(option),
                })
            })
    }
}

impl FeatureOptionPack for InputBin {
    fn option_feature_path(&self) -> &[OwnedName] {
        self.feature_path
    }

    fn option(&self) -> &PrintFeatureOption {
        &self.option
    }

    fn option_mut(&mut self) -> &mut PrintFeatureOption {
        &mut self.option
    }

    fn parameters(&self) -> &[ParameterInit] {
        &self.parameters
    }

    fn parameters_mut(&mut self) -> &mut Vec<ParameterInit> {
        &mut self.parameters
    }

    fn into_option_with_parameters(self) -> (PrintFeatureOption, Vec<ParameterInit>) {
        (self.option, self.parameters)
    }
}

impl FeatureOptionPackWithPredefined for InputBin {
    type PredefinedName = PredefinedInputBin;
}

#[cfg(test)]
mod tests {
    use crate::{
        test_utils::null_device,
        ticket::{
            document::{OwnedName, NS_PSK},
            test_helpers::{assert_converted, assert_merged, capabilities_with_features},
            FeatureOptionPack, FeatureOptionPackWithPredefined, PredefinedInputBin,
            PrintCapabilities,
        },
    };

    #[test]
    fn use_auto_select() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let auto_select = capabilities
            .input_bins()
            .find(|x| x.as_predefined_name() == Some(PredefinedInputBin::AutoSelect))
            .unwrap();
        assert_merged(&device, auto_select);
    }

    #[test]
    fn input_bins_prefer_the_widest_scope() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:JobInputBin">
        <psf:Option name="psk:Cassette" />
    </psf:Feature>
    <psf:Feature name="psk:DocumentInputBin">
        <psf:Option name="psk:Manual" />
        <psf:Option name="psk:Tractor" />
    </psf:Feature>
    <psf:Feature name="psk:PageInputBin">
        <psf:Option name="psk:AutoSheetFeeder" />
    </psf:Feature>"#,
        );

        let bins = capabilities.input_bins().collect::<Vec<_>>();
        assert_eq!(bins.len(), 1);
        assert_eq!(
            bins[0].as_predefined_name(),
            Some(PredefinedInputBin::Cassette)
        );
        assert_eq!(
            bins[0].option_feature_path(),
            vec![OwnedName::qualified("JobInputBin", NS_PSK, Some("psk"))]
        );
    }

    #[test]
    fn input_bins_fall_back_to_narrower_scopes_only_when_needed() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:DocumentInputBin">
        <psf:Option name="psk:Manual" />
        <psf:Option name="psk:Tractor" />
    </psf:Feature>
    <psf:Feature name="psk:PageInputBin">
        <psf:Option name="psk:AutoSheetFeeder" />
    </psf:Feature>"#,
        );

        let bins = capabilities.input_bins().collect::<Vec<_>>();
        assert_eq!(
            bins.iter()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            vec![
                Some(PredefinedInputBin::Manual),
                Some(PredefinedInputBin::Tractor)
            ]
        );
        let document_input_bin = vec![OwnedName::qualified(
            "DocumentInputBin",
            NS_PSK,
            Some("psk"),
        )];
        assert!(bins
            .iter()
            .all(|x| x.option_feature_path() == document_input_bin));

        // the option is converted into the print ticket as the feature that defined it
        bins.into_iter().for_each(assert_converted);

        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:PageInputBin">
        <psf:Option name="psk:AutoSheetFeeder" />
    </psf:Feature>"#,
        );
        assert_eq!(
            capabilities
                .input_bins()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            vec![Some(PredefinedInputBin::AutoSheetFeeder)]
        );
    }
}
