use std::fmt;
use xml::name::OwnedName;

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub enum PrintSchemaDocument {
    PrintCapabilities(PrintCapabilitiesDocument),
    PrintTicket(PrintTicketDocument),
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct PrintCapabilitiesDocument {
    pub properties: Vec<Property>,
    pub parameter_defs: Vec<ParameterDef>,
    pub features: Vec<PrintFeature>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct PrintTicketDocument {
    pub properties: Vec<Property>,
    pub parameter_inits: Vec<ParameterInit>,
    pub features: Vec<PrintFeature>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct PrintFeature {
    #[fmt("{}", self.name)]
    pub name: OwnedName,
    pub properties: Vec<Property>,
    pub options: Vec<PrintFeatureOption>,
    pub features: Vec<PrintFeature>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct ParameterInit {
    #[fmt("{}", self.name)]
    pub name: OwnedName,
    #[fmt("{:?}", self.value)]
    pub value: PropertyValue,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct ParameterDef {
    #[fmt("{}", self.name)]
    pub name: OwnedName,
    pub properties: Vec<Property>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct PrintFeatureOption {
    #[fmt("{}", self.name.as_ref().map(|x| x.to_string()).unwrap_or("<unnamed>".to_string()))]
    pub name: Option<OwnedName>,
    pub scored_properties: Vec<ScoredProperty>,
    pub properties: Vec<Property>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct ScoredProperty {
    #[fmt("{}", self.name.as_ref().map(|x| x.to_string()).unwrap_or("<unnamed>".to_string()))]
    pub name: Option<OwnedName>,
    #[fmt("{}", self.parameter_ref.as_ref().map(|x| x.to_string()).unwrap_or("<unnamed>".to_string()))]
    pub parameter_ref: Option<OwnedName>,
    #[fmt("{}", self.value.as_ref().map(|x| format!("{:?}", x)).unwrap_or("<none>".to_string()))]
    pub value: Option<PropertyValue>,
    pub scored_properties: Vec<ScoredProperty>,
    pub properties: Vec<Property>,
}

#[derive(Clone, PartialEq, Eq, Hash, fmt_derive::Debug)]
pub struct Property {
    #[fmt("{}", self.name)]
    pub name: OwnedName,
    #[fmt("{}", self.value.as_ref().map(|x| format!("{:?}", x)).unwrap_or("<none>".to_string()))]
    pub value: Option<PropertyValue>,
    pub properties: Vec<Property>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum PropertyValue {
    String(String),
    Integer(i32),
    QName(OwnedName),
    Unknown(OwnedName, String),
}

impl fmt::Debug for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyValue::String(s) => write!(f, "String({:?})", s),
            PropertyValue::Integer(i) => write!(f, "Integer({})", i),
            PropertyValue::QName(q) => write!(f, "QName({})", q),
            PropertyValue::Unknown(n, s) => write!(f, "Unknown({}, {:?})", n, s),
        }
    }
}

impl ParameterDef {
    pub fn default_value(&self) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|x| {
                x.name.local_name == "DefaultValue" && x.name.namespace_ref() == Some(super::NS_PSF)
            })
            .and_then(|x| x.value.as_ref())
    }
}

impl PropertyValue {
    pub fn xsi_type(&self) -> OwnedName {
        match self {
            PropertyValue::String(_) => OwnedName::qualified("string", super::NS_XSD, Some("xsd")),
            PropertyValue::Integer(_) => {
                OwnedName::qualified("integer", super::NS_XSD, Some("xsd"))
            }
            PropertyValue::QName(_) => OwnedName::qualified("QName", super::NS_XSD, Some("xsd")),
            PropertyValue::Unknown(n, _) => n.clone(),
        }
    }
    pub fn string(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn integer(&self) -> Option<i32> {
        match self {
            PropertyValue::Integer(i) => Some(*i),
            _ => None,
        }
    }
    pub fn qualified_name(&self) -> Option<&OwnedName> {
        match self {
            PropertyValue::QName(q) => Some(q),
            _ => None,
        }
    }
}

impl PrintFeatureOption {
    pub fn parameters_dependent(&self) -> Vec<OwnedName> {
        let mut result = vec![];
        for scored_property in &self.scored_properties {
            result.extend(scored_property.parameters_dependent());
        }
        result
    }
}

impl ScoredProperty {
    pub fn parameters_dependent(&self) -> Vec<OwnedName> {
        let mut result = vec![];
        if let Some(ref parameter_ref) = self.parameter_ref {
            result.push(parameter_ref.clone());
        }
        for scored_property in &self.scored_properties {
            result.extend(scored_property.parameters_dependent());
        }
        result
    }
    pub fn value_with<'a>(&'a self, parameters: &'a [ParameterInit]) -> Option<&'a PropertyValue> {
        if let Some(ref parameter_ref) = self.parameter_ref {
            parameters
                .iter()
                .find(|x| x.name == *parameter_ref)
                .map(|x| &x.value)
        } else {
            self.value.as_ref()
        }
    }
}

impl From<PrintTicketDocument> for PrintSchemaDocument {
    fn from(value: PrintTicketDocument) -> Self {
        PrintSchemaDocument::PrintTicket(value)
    }
}

impl From<PrintCapabilitiesDocument> for PrintSchemaDocument {
    fn from(value: PrintCapabilitiesDocument) -> Self {
        PrintSchemaDocument::PrintCapabilities(value)
    }
}

pub trait WithScoredProperties {
    fn scored_properties(&self) -> &[ScoredProperty];
    fn get_scored_property(&self, name: &str, namespace: Option<&str>) -> Option<&ScoredProperty> {
        self.scored_properties().iter().find(|x| {
            x.name.as_ref().map_or(false, |x| {
                x.local_name == name && x.namespace_ref() == namespace
            })
        })
    }
}

impl WithScoredProperties for PrintFeatureOption {
    fn scored_properties(&self) -> &[ScoredProperty] {
        &self.scored_properties
    }
}

impl WithScoredProperties for ScoredProperty {
    fn scored_properties(&self) -> &[ScoredProperty] {
        &self.scored_properties
    }
}

pub trait WithProperties {
    fn properties(&self) -> &[Property];
    fn get_property(&self, name: &str, namespace: Option<&str>) -> Option<&Property> {
        self.properties()
            .iter()
            .find(|x| x.name.local_name == name && x.name.namespace_ref() == namespace)
    }
}

impl WithProperties for PrintSchemaDocument {
    fn properties(&self) -> &[Property] {
        match self {
            PrintSchemaDocument::PrintCapabilities(x) => x.properties(),
            PrintSchemaDocument::PrintTicket(x) => x.properties(),
        }
    }
}

impl WithProperties for PrintCapabilitiesDocument {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for PrintTicketDocument {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for ParameterDef {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for PrintFeature {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for PrintFeatureOption {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for ScoredProperty {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}

impl WithProperties for Property {
    fn properties(&self) -> &[Property] {
        &self.properties
    }
}
