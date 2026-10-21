use xml::name::OwnedName;

use super::{
    document::{
        ParameterInit, PrintFeature, PrintFeatureOption, PrintTicketDocument, WithProperties,
        NS_PSK,
    },
    PrintCapabilities, PrintTicket,
};

/// A trait for the predefined name.
pub trait PredefinedName: Sized {
    /// Get the predefined name from the given name.
    /// If the name is not predefined, `None` is returned.
    fn from_name(name: &OwnedName) -> Option<Self>;
}

/// A trait for a pack that carries one option of a feature.
///
/// The feature is identified by a path of names, from the outermost feature to the
/// innermost sub-feature. A pack such as [`InputBin`](crate::ticket::InputBin) may
/// carry options of different features, so the path is only known per instance.
/// Packs that are always an option of one fixed feature path also implement
/// [`StaticFeaturePathOptionPack`].
pub trait FeatureOptionPack: Sized {
    /// Get the path of features that the option of this pack belongs to.
    ///
    /// The names are namespace-qualified, ordered from the outermost feature to
    /// the innermost sub-feature. When the path has more than one name, converting
    /// the pack into a [`PrintTicket`] nests the option under its ancestors, which
    /// then carry no option of their own.
    fn option_feature_path(&self) -> &[OwnedName];

    /// Get the DOM of the option.
    fn option(&self) -> &PrintFeatureOption;
    /// Get the mutable reference to the DOM of the option.
    fn option_mut(&mut self) -> &mut PrintFeatureOption;

    /// Get the parameters that is used by the option.
    fn parameters(&self) -> &[ParameterInit];
    /// Get the mutable reference to the parameters that is used by the option.
    fn parameters_mut(&mut self) -> &mut Vec<ParameterInit>;

    /// Convert the feature option pack into the option and the parameters.
    fn into_option_with_parameters(self) -> (PrintFeatureOption, Vec<ParameterInit>);

    /// Get display name of the page orientation.
    fn display_name(&self) -> Option<&str> {
        self.option()
            .get_property("DisplayName", Some(NS_PSK))
            .and_then(|x| x.value.as_ref())
            .and_then(|x| x.string())
    }
}

/// A trait for the feature option pack whose feature path is known statically.
///
/// Such a pack is always an option of one fixed feature path, so that it can be
/// created from an option and can list all options that the capabilities document
/// defines for that feature.
pub trait StaticFeaturePathOptionPack: FeatureOptionPack {
    /// Create a new instance.
    fn new(option: PrintFeatureOption, parameters: Vec<ParameterInit>) -> Self;

    /// Get the path of features that the option of this pack belongs to.
    fn feature_path() -> &'static [OwnedName];

    /// Create a new instance from the given option, filling in the default values
    /// of the parameters that the option depends on.
    fn from_option(capabilities: &PrintCapabilities, option: &PrintFeatureOption) -> Self {
        Self::new(
            option.clone(),
            capabilities.default_parameters_for_option(option),
        )
    }

    /// List all possible options defined in the capabilities.
    fn list(capabilities: &PrintCapabilities) -> impl Iterator<Item = Self> + '_ {
        capabilities
            .options_for_feature(Self::feature_path())
            .map(|option| Self::from_option(capabilities, option))
    }
}

impl<T> From<T> for PrintTicket
where
    T: FeatureOptionPack,
{
    fn from(value: T) -> Self {
        let feature_path = value.option_feature_path().to_vec();
        let (option, parameters) = value.into_option_with_parameters();
        // Nest the option under its ancestors, from the innermost one outwards.
        let features = feature_path
            .split_last()
            .map(|(leaf, ancestors)| {
                ancestors.iter().rev().fold(
                    PrintFeature {
                        name: leaf.clone(),
                        properties: vec![],
                        options: vec![option],
                        features: vec![],
                    },
                    |feature, name| PrintFeature {
                        name: name.clone(),
                        properties: vec![],
                        options: vec![],
                        features: vec![feature],
                    },
                )
            })
            .into_iter()
            .collect();
        PrintTicketDocument {
            properties: vec![],
            parameter_inits: parameters,
            features,
        }
        .into()
    }
}

/// A trait for the feature option pack with predefined name.
pub trait FeatureOptionPackWithPredefined: FeatureOptionPack {
    /// The type which represents the predefined name.
    type PredefinedName: PredefinedName;

    /// Get the predefined name of the option.
    /// If the option is not predefined, `None` is returned.
    fn as_predefined_name(&self) -> Option<Self::PredefinedName> {
        self.option()
            .name
            .as_ref()
            .and_then(Self::PredefinedName::from_name)
    }
}

/// Implement the feature option pack traits for the given type.
///
/// This implements [`FeatureOptionPack`] and, since the feature path is given by
/// `$feature_path`, [`StaticFeaturePathOptionPack`].
///
/// # Parameters
/// - `$feature_path:expr`: A vector of namespace-qualified feature names, from the
///   outermost feature to the innermost sub-feature.
/// - `$name:ident`: The type to define.
/// - `$predefined_name:ident`: The type of predefined name. If not specified, the type is not predefined.
///
/// # Example
/// ```ignore
/// define_feature_option_pack!(
///     vec![OwnedName::qualified("MyFeature", NS_PSK, Some("psk"))],
///     MyPack,
///     MyPredefinedName
/// );
/// ```
macro_rules! define_feature_option_pack {
    ($feature_path:expr, $name:ident) => {
        #[derive(Clone, Debug)]
        #[doc = concat!("Represents a feature option pack as [`", stringify!($name), "`].")]
        pub struct $name {
            /// The option of the feature.
            option: PrintFeatureOption,
            /// The parameters that is used by the option.
            parameters: Vec<ParameterInit>,
        }

        impl crate::ticket::FeatureOptionPack for $name {
            fn option_feature_path(&self) -> &[xml::name::OwnedName] {
                <Self as crate::ticket::StaticFeaturePathOptionPack>::feature_path()
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

        impl crate::ticket::StaticFeaturePathOptionPack for $name {
            fn new(option: PrintFeatureOption, parameters: Vec<ParameterInit>) -> Self {
                Self { option, parameters }
            }

            fn feature_path() -> &'static [xml::name::OwnedName] {
                static PATH: std::sync::LazyLock<Vec<xml::name::OwnedName>> =
                    std::sync::LazyLock::new(|| $feature_path);
                &PATH
            }
        }
    };
    ($feature_path:expr, $name:ident, $predefined_name:ident) => {
        define_feature_option_pack!($feature_path, $name);

        impl crate::ticket::FeatureOptionPackWithPredefined for $name {
            type PredefinedName = $predefined_name;
        }
    };
}
pub(crate) use define_feature_option_pack;
