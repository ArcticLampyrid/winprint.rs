use super::{
    document::{
        reader::{ParsableXmlDocument, ParsePrintSchemaError},
        ParameterInit, PrintCapabilitiesDocument, PrintFeature, PrintFeatureOption, WithProperties,
        NS_PSF, NS_PSK,
    },
    Copies, JobDuplex, PageMediaSize, PageOrientation, PageOutputColor, PageResolution,
    PrintTicket, StaticFeaturePathOptionPack,
};
use crate::{
    printer::PrinterDevice,
    utils::{stream::read_com_stream, wchar},
};
use scopeguard::defer;
use std::fmt::Debug;
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Graphics::Printing::PrintTicket::{
            PTCloseProvider, PTGetPrintCapabilities, PTOpenProvider,
        },
        UI::Shell::SHCreateMemStream,
    },
};
use xml::name::OwnedName;

#[derive(Error, Debug)]
/// Represents an error occurred while fetching print capabilities.
pub enum FetchPrintCapabilitiesError {
    /// Failed to open print ticket provider.
    #[error("Failed to open print ticket provider")]
    OpenProviderFailed(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Cannot get print capabilities.
    #[error("Cannot get print capabilities")]
    CannotGetPrintCapabilities(String, #[source] windows::core::Error),
    /// Failed to read stream.
    #[error("Failed to read stream")]
    ReadStreamFailed(#[source] windows::core::Error),
    /// Failed to parse print capabilities.
    #[error("Failed to parse print capabilities")]
    ParseError(#[source] ParsePrintSchemaError),
}

#[derive(Clone, Debug)]
/// Represents print capabilities.
pub struct PrintCapabilities {
    /// DOM of print capabilities document.
    pub document: PrintCapabilitiesDocument,
}

impl PrintCapabilities {
    /// Fetch print capabilities XML (without parsing it) for the given printer device.
    pub fn fetch_xml(device: &PrinterDevice) -> Result<Vec<u8>, FetchPrintCapabilitiesError> {
        Self::fetch_xml_for_ticket(device, None)
    }

    /// Fetch print capabilities XML (without parsing it) for the given printer device and print ticket.
    pub fn fetch_xml_for_ticket(
        device: &PrinterDevice,
        ticket: Option<&PrintTicket>,
    ) -> Result<Vec<u8>, FetchPrintCapabilitiesError> {
        unsafe {
            let provider =
                PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1)
                    .map_err(FetchPrintCapabilitiesError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }
            let stream =
                SHCreateMemStream(None).ok_or(FetchPrintCapabilitiesError::StreamNotAllocated)?;
            let ticket_stream = ticket
                .map(|x| {
                    SHCreateMemStream(Some(x.get_xml()))
                        .ok_or(FetchPrintCapabilitiesError::StreamNotAllocated)
                })
                .transpose()?;

            let mut error_message = BSTR::default();
            PTGetPrintCapabilities(
                provider,
                ticket_stream.as_ref(),
                &stream,
                Some(&mut error_message),
            )
            .map_err(|win32_error| {
                FetchPrintCapabilitiesError::CannotGetPrintCapabilities(
                    error_message.to_string(),
                    win32_error,
                )
            })?;

            let data =
                read_com_stream(&stream).map_err(FetchPrintCapabilitiesError::ReadStreamFailed)?;

            Ok(data)
        }
    }

    /// Fetch and parse print capabilities for the given printer device.
    pub fn fetch(device: &PrinterDevice) -> Result<PrintCapabilities, FetchPrintCapabilitiesError> {
        let xml = Self::fetch_xml(device)?;
        let document = PrintCapabilitiesDocument::parse_from_bytes(xml)
            .map_err(FetchPrintCapabilitiesError::ParseError)?;
        Ok(PrintCapabilities { document })
    }

    /// Defines all parameters with default values.
    pub fn default_parameters(&self) -> impl Iterator<Item = ParameterInit> + '_ {
        self.document.parameter_defs.iter().filter_map(|param_def| {
            param_def
                .default_value()
                .map(|default_value| ParameterInit {
                    name: param_def.name.clone(),
                    value: default_value.clone(),
                })
        })
    }

    /// Defines the given parameters with default values.
    pub fn default_parameters_for<'a>(
        &'a self,
        filters: &'a [OwnedName],
    ) -> impl Iterator<Item = ParameterInit> + 'a {
        let mut filters = filters
            .iter()
            .map(|x| (&x.namespace, &x.local_name))
            .collect::<Vec<_>>();
        filters.sort_unstable();
        self.document
            .parameter_defs
            .iter()
            .filter(move |param_def| {
                filters
                    .binary_search(&(&param_def.name.namespace, &param_def.name.local_name))
                    .is_ok()
            })
            .filter_map(move |param_def| {
                param_def
                    .default_value()
                    .map(|default_value| ParameterInit {
                        name: param_def.name.clone(),
                        value: default_value.clone(),
                    })
            })
    }

    /// Defines the parameters that the given option depends on with default values.
    pub(crate) fn default_parameters_for_option(
        &self,
        option: &PrintFeatureOption,
    ) -> Vec<ParameterInit> {
        self.default_parameters_for(option.parameters_dependent().as_slice())
            .collect()
    }

    /// Get all options for the given feature path.
    ///
    /// The path consists of namespace-qualified names, ordered from the outermost
    /// feature to the innermost sub-feature. Names are matched by namespace URI and
    /// local name, ignoring prefixes.
    pub fn options_for_feature(
        &self,
        feature_path: &[OwnedName],
    ) -> impl Iterator<Item = &PrintFeatureOption> + '_ {
        features_at_path(&self.document.features, feature_path)
            .into_iter()
            .flat_map(|x| x.options.iter())
    }

    /// Get all page media sizes.
    pub fn page_media_sizes(&self) -> impl Iterator<Item = PageMediaSize> + '_ {
        PageMediaSize::list(self)
    }

    /// Get all supported page orientations.
    pub fn page_orientations(&self) -> impl Iterator<Item = PageOrientation> + '_ {
        PageOrientation::list(self)
    }

    /// Get all supported job duplex types.
    ///
    /// # Note
    /// This corresponds to the Print Schema's `JobDuplexAllDocumentsContiguously` keyword, not the `DocumentDuplex` keyword.
    pub fn duplexes(&self) -> impl Iterator<Item = JobDuplex> + '_ {
        JobDuplex::list(self)
    }

    /// Get all supported page output colors.
    pub fn page_output_colors(&self) -> impl Iterator<Item = PageOutputColor> + '_ {
        PageOutputColor::list(self)
    }

    /// Get all supported page resolutions.
    pub fn page_resolutions(&self) -> impl Iterator<Item = PageResolution> + '_ {
        PageResolution::list(self)
    }

    /// Get the maximum number of copies that a printer can print. Return `None` if the device does not report a maximum.
    ///
    /// # Note
    /// This corresponds to the Print Schema's `JobCopiesAllDocuments` keyword, not the `DocumentCopiesAllPages` keyword, or the `PageCopies` keyword. If the printer can print unlimited copies, the property value is 9999.
    pub fn max_copies(&self) -> Option<Copies> {
        self.document
            .parameter_defs
            .iter()
            .find(|x| {
                x.name.local_name == "JobCopiesAllDocuments"
                    && x.name.namespace_ref() == Some(NS_PSK)
            })
            .and_then(|x| x.get_property("MaxValue", Some(NS_PSF)))
            .and_then(|x| x.value.as_ref())
            .and_then(|x| x.integer())
            .and_then(|x| u16::try_from(x).ok())
            .map(Copies)
    }
}

/// Get the features that the given path points to.
///
/// The names of the path are namespace-qualified, ordered from the outermost
/// feature to the innermost sub-feature.
pub(crate) fn features_at_path<'a>(
    features: &'a [PrintFeature],
    feature_path: &[OwnedName],
) -> Vec<&'a PrintFeature> {
    let Some((first, rest)) = feature_path.split_first() else {
        return vec![];
    };
    rest.iter().fold(
        features
            .iter()
            .filter(|x| name_matches(&x.name, first))
            .collect(),
        |level, name| {
            level
                .into_iter()
                .flat_map(|x| &x.features)
                .filter(|x| name_matches(&x.name, name))
                .collect()
        },
    )
}

/// Check whether two names are the same, by namespace URI and local name,
/// ignoring prefixes.
pub(crate) fn name_matches(a: &OwnedName, b: &OwnedName) -> bool {
    a.local_name == b.local_name && a.namespace == b.namespace
}

#[cfg(test)]
mod tests {
    use super::PrintCapabilities;
    use crate::{
        test_utils::null_device,
        ticket::{
            define_feature_option_pack,
            document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
            test_helpers::{assert_converted, capabilities_from_xml, capabilities_with_features},
            StaticFeaturePathOptionPack,
        },
    };

    #[test]
    fn test_fetch_xml() {
        let device = null_device::thread_local();
        PrintCapabilities::fetch_xml(&device).unwrap();
    }

    #[test]
    fn test_fetch_xml_and_parse() {
        let device = null_device::thread_local();
        PrintCapabilities::fetch(&device).unwrap();
    }

    #[test]
    fn feature_paths_match_namespaces_not_prefixes() {
        let capabilities = capabilities_with_features(
            r#"<psf:Feature name="psk:Root">
    <psf:Option name="psk:PublicRoot" />
    <psf:Feature name="psk:Child">
        <psf:Option name="psk:PublicChild" />
    </psf:Feature>
    <psf:Feature xmlns:v="urn:vendor" name="v:Child">
        <psf:Option name="v:PrivateChild" />
    </psf:Feature>
</psf:Feature>
<psf:Feature xmlns:v="urn:vendor" name="v:Root">
    <psf:Option name="v:PrivateRoot" />
</psf:Feature>"#,
        );
        let public_root = OwnedName::qualified("Root", NS_PSK, Some("keywords"));
        let private_root = OwnedName::qualified("Root", "urn:vendor", Some("vendor"));
        let public_child = OwnedName::qualified("Child", NS_PSK, Some("keywords"));
        let private_child = OwnedName::qualified("Child", "urn:vendor", Some("vendor"));
        for (path, expected) in [
            (vec![public_root.clone()], "PublicRoot"),
            (vec![private_root], "PrivateRoot"),
            (vec![public_root.clone(), public_child], "PublicChild"),
            (vec![public_root, private_child], "PrivateChild"),
        ] {
            let options = capabilities.options_for_feature(&path).collect::<Vec<_>>();
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].name.as_ref().unwrap().local_name, expected);
        }
        assert_eq!(
            capabilities
                .options_for_feature(&[OwnedName::qualified("Root", "urn:other", Some("psk"))])
                .count(),
            0
        );
        assert_eq!(capabilities.options_for_feature(&[]).count(), 0);
    }

    #[test]
    fn feature_paths_reach_vendor_sub_features_from_driver_output() {
        let capabilities =
            capabilities_from_xml(include_bytes!("../../test_data/print_capabilities.xml"));
        let parent = capabilities
            .document
            .features
            .iter()
            .find(|x| x.name.local_name == "JobNUpAllDocumentsContiguously")
            .unwrap();
        let borders = parent
            .features
            .iter()
            .find(|x| x.name.local_name == "Borders")
            .unwrap();
        let mut path = vec![OwnedName::qualified(
            "JobNUpAllDocumentsContiguously",
            NS_PSK,
            Some("psk"),
        )];
        path.push(borders.name.clone());
        assert_eq!(
            capabilities
                .options_for_feature(&path)
                .map(|x| x.name.as_ref().unwrap().local_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Off", "On"]
        );
    }

    #[test]
    fn custom_packs_preserve_feature_namespaces_in_print_tickets() {
        define_feature_option_pack!(
            vec![OwnedName::qualified("Root", "urn:vendor", Some("vendor"))],
            VendorRoot
        );
        define_feature_option_pack!(
            vec![
                OwnedName::qualified("Root", NS_PSK, Some("psk")),
                OwnedName::qualified("Child", "urn:vendor", Some("vendor")),
            ],
            VendorChild
        );
        let capabilities = capabilities_with_features(
            r#"<psf:Feature xmlns:v="urn:vendor" name="v:Root">
    <psf:Option name="v:Enabled" />
</psf:Feature>
<psf:Feature name="psk:Root">
    <psf:Feature xmlns:v="urn:vendor" name="v:Child">
        <psf:Option name="v:Enabled" />
    </psf:Feature>
</psf:Feature>"#,
        );
        assert_eq!(VendorRoot::list(&capabilities).count(), 1);
        VendorRoot::list(&capabilities).for_each(assert_converted);
        assert_eq!(VendorChild::list(&capabilities).count(), 1);
        VendorChild::list(&capabilities).for_each(assert_converted);
    }
}
