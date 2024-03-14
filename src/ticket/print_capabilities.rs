use super::{
    document::{
        reader::{ParsableXmlDocument, ParsePrintSchemaError},
        ParameterInit, PrintCapabilitiesDocument, PrintFeatureOption, NS_PSK,
    },
    PageMediaSize,
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
    #[error("Failed to open print ticket provider: {0}")]
    OpenProviderFailed(windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Cannot get print capabilities.
    #[error("Cannot get print capabilities: {0}")]
    CannotGetPrintCapabilities(String, windows::core::Error),
    /// Failed to read stream.
    #[error("Failed to read stream: {0}")]
    ReadStreamFailed(windows::core::Error),
    /// Failed to parse print capabilities.
    #[error("Failed to parse print capabilities: {0}")]
    ParseError(ParsePrintSchemaError),
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
        unsafe {
            let provider =
                PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1)
                    .map_err(FetchPrintCapabilitiesError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }
            let stream =
                SHCreateMemStream(None).ok_or(FetchPrintCapabilitiesError::StreamNotAllocated)?;

            let mut error_message = BSTR::default();
            PTGetPrintCapabilities(provider, None, &stream, Some(&mut error_message)).map_err(
                |win32_error| {
                    FetchPrintCapabilitiesError::CannotGetPrintCapabilities(
                        error_message.to_string(),
                        win32_error,
                    )
                },
            )?;

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

    /// Get all options for the given feature.
    pub fn options_for_feature<'a>(
        &'a self,
        feature_name: &'a str,
        namespace: Option<&'a str>,
    ) -> impl Iterator<Item = &PrintFeatureOption> + 'a {
        self.document
            .features
            .iter()
            .filter(move |x| {
                x.name.local_name == feature_name && x.name.namespace_ref() == namespace
            })
            .flat_map(|x| x.options.iter())
    }

    fn collect_default_parameters_for_option(
        &self,
        option: &PrintFeatureOption,
    ) -> Vec<ParameterInit> {
        self.default_parameters_for(option.parameters_dependent().as_slice())
            .collect()
    }

    /// Get all page media sizes.
    pub fn page_media_size(&self) -> impl Iterator<Item = PageMediaSize> + '_ {
        self.options_for_feature("PageMediaSize", Some(NS_PSK))
            .map(|option| {
                PageMediaSize::new(
                    option.clone(),
                    self.collect_default_parameters_for_option(option),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::PrintCapabilities;
    use crate::test_utils::null_device;
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
}
