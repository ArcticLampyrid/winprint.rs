use super::{
    define_feature_option_pack,
    document::{
        OwnedName, ParameterInit, PrintFeatureOption, PropertyValue, WithScoredProperties, NS_PSK,
    },
    print_capabilities::name_matches,
    PredefinedPageDestinationColorProfile,
};

define_feature_option_pack!(
    vec![OwnedName::qualified(
        "PageDestinationColorProfile",
        NS_PSK,
        Some("psk")
    )],
    PageDestinationColorProfile,
    PredefinedPageDestinationColorProfile
);

/// The option does not expose the requested destination profile property.
#[derive(Debug, thiserror::Error)]
#[error("The option does not expose {0}")]
pub struct DestinationColorProfilePropertyNotSupported(&'static str);

impl PageDestinationColorProfile {
    /// Get the destination profile URI, if this option specifies one.
    pub fn destination_color_profile_uri(&self) -> Option<&str> {
        self.profile_property("DestinationColorProfileURI")
    }

    /// Set the destination profile URI exposed by this option.
    ///
    /// Updates the advertised scored property or the parameter it references,
    /// initializing the parameter if needed. Returns an error without changing the
    /// pack if the property is absent, as it normally is for `DriverConfiguration`.
    /// Does not change the selected option.
    ///
    /// For XPS, use the part name of an ICC profile contained in the document package,
    /// such as `/Resources/ColorProfiles/printer.icc`. This method does not load or
    /// embed the profile, validate the URI, or perform color conversion.
    ///
    /// Specify only one profile source. If the option also carries
    /// `DestinationColorProfileEmbedded`, remove that scored property before using
    /// a URI.
    pub fn set_destination_color_profile_uri(
        &mut self,
        uri: impl Into<String>,
    ) -> Result<(), DestinationColorProfilePropertyNotSupported> {
        self.set_profile_property("DestinationColorProfileURI", uri.into())
    }

    /// Get the base64-encoded destination profile, if this option specifies one.
    ///
    /// The returned string is not decoded or validated.
    pub fn destination_color_profile_embedded(&self) -> Option<&str> {
        self.profile_property("DestinationColorProfileEmbedded")
    }

    /// Set the base64-encoded ICC profile exposed by this option.
    ///
    /// Updates the advertised scored property or the parameter it references,
    /// initializing the parameter if needed. Returns an error without changing the
    /// pack if the property is absent. Does not change the selected option.
    ///
    /// The caller must base64-encode the profile bytes. This method does not encode
    /// or validate the data, or perform color conversion.
    ///
    /// Specify only one profile source. If the option also carries
    /// `DestinationColorProfileURI`, remove that scored property before embedding
    /// a profile. Prefer a URI for GDI printing: embedded profiles can exceed the
    /// driver's DEVMODE size limits.
    pub fn set_destination_color_profile_embedded(
        &mut self,
        base64_profile: impl Into<String>,
    ) -> Result<(), DestinationColorProfilePropertyNotSupported> {
        self.set_profile_property("DestinationColorProfileEmbedded", base64_profile.into())
    }

    fn profile_property(&self, property_name: &str) -> Option<&str> {
        let property = self
            .option
            .get_scored_property(property_name, Some(NS_PSK))?;
        let value = match &property.parameter_ref {
            Some(name) => {
                &self
                    .parameters
                    .iter()
                    .find(|parameter| name_matches(&parameter.name, name))?
                    .value
            }
            None => property.value.as_ref()?,
        };
        value.string()
    }

    fn set_profile_property(
        &mut self,
        property_name: &'static str,
        value: String,
    ) -> Result<(), DestinationColorProfilePropertyNotSupported> {
        let property = self
            .option
            .scored_properties
            .iter_mut()
            .find(|property| {
                property.name.as_ref().is_some_and(|name| {
                    name.namespace_ref() == Some(NS_PSK) && name.local_name == property_name
                })
            })
            .ok_or(DestinationColorProfilePropertyNotSupported(property_name))?;
        let value = PropertyValue::String(value);
        if let Some(name) = &property.parameter_ref {
            if let Some(parameter) = self
                .parameters
                .iter_mut()
                .find(|parameter| name_matches(&parameter.name, name))
            {
                parameter.value = value;
            } else {
                self.parameters.push(ParameterInit {
                    name: name.clone(),
                    value,
                });
            }
        } else {
            property.value = Some(value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::ticket::{
        test_helpers::{assert_converted, capabilities_with_features},
        FeatureOptionPackWithPredefined, PredefinedPageDestinationColorProfile,
    };

    #[test]
    fn lists_and_converts_every_predefined_option() {
        let capabilities = capabilities_with_features(
            r#"    <psf:Feature name="psk:PageDestinationColorProfile">
        <psf:Option name="psk:Application" />
        <psf:Option name="psk:DriverConfiguration" />
        <psf:Option name="ns0000:Vendor" xmlns:ns0000="http://example.com/vendor" />
    </psf:Feature>"#,
        );
        let options = capabilities
            .page_destination_color_profiles()
            .collect::<Vec<_>>();
        assert_eq!(
            options
                .iter()
                .map(|x| x.as_predefined_name())
                .collect::<Vec<_>>(),
            [
                Some(PredefinedPageDestinationColorProfile::Application),
                Some(PredefinedPageDestinationColorProfile::DriverConfiguration),
                None
            ]
        );
        options.into_iter().for_each(assert_converted);
    }
}
