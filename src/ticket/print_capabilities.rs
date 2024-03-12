use super::{
    PageMediaSize, ParameterInit, ParsePrintSchemaError, PrintCapabilitiesDocument,
    PrintFeatureOption, PrintSchemaDocument, NS_PSK,
};
use crate::{
    printer::PrinterInfo,
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
pub enum FetchPrintCapabilitiesError {
    #[error("Failed to open print ticket provider: {0}")]
    OpenProviderFailed(windows::core::Error),
    #[error("Stream not allocated")]
    StreamNotAllocated,
    #[error("Cannot get print capabilities: {0}")]
    CannotGetPrintCapabilities(String, windows::core::Error),
    #[error("Failed to read stream: {0}")]
    ReadStreamFailed(windows::core::Error),
    #[error("Failed to parse print capabilities: {0}")]
    ParseError(ParsePrintSchemaError),
}

#[derive(Clone, Debug)]
pub struct PrintCapabilities {
    pub document: PrintCapabilitiesDocument,
}

impl PrintCapabilities {
    pub fn fetch_xml(info: &PrinterInfo) -> Result<Vec<u8>, FetchPrintCapabilitiesError> {
        unsafe {
            let provider = PTOpenProvider(PCWSTR(wchar::to_wide_chars(info.os_name()).as_ptr()), 1)
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

    pub fn fetch(info: &PrinterInfo) -> Result<PrintCapabilities, FetchPrintCapabilitiesError> {
        let xml = Self::fetch_xml(info)?;
        let document = PrintSchemaDocument::parse_as_capabilities(xml)
            .map_err(FetchPrintCapabilitiesError::ParseError)?;
        Ok(PrintCapabilities { document })
    }

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
    use crate::tests::get_test_printer;
    #[test]
    fn test_fetch_xml() {
        let test_printer = get_test_printer();
        PrintCapabilities::fetch_xml(&test_printer).unwrap();
    }

    #[test]
    fn test_fetch_xml_and_parse() {
        let test_printer = get_test_printer();
        PrintCapabilities::fetch(&test_printer).unwrap();
    }
}
