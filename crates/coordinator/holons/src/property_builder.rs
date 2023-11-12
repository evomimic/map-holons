#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct PropertyBuilder {
    pub header: Option<TypeHeaderBuilder>,
    pub descriptor_sharing: Option<PropertySharing>
    pub details: Option<PropertyDetailsBuilder>,
}

pub struct PropertyUsageBuilder {
    pub description: Option<String>,
    pub descriptor: Option<Property>,
}

pub struct PropertyMap {
    pub properties: Option<BTreeMap<String, PropertyUsage>>,
}

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PropertyDescriptorDetailsBuilder {
    Boolean(BooleanDescriptorBuilder),
    Composite(CompositeDescriptorBuilder),
    //Enum(EnumDescriptorBuilder),
    Integer(IntegerDescriptorBuilder),
    String(StringDescriptorBuilder),
    ValueCollection(ValueCollectionDescriptorBuilder), // can only contain collections of PropertyTypes (not Holons)
}

#[hdk_entry_helper]
#[derive(new, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BooleanDescriptorBuilder {
    pub is_fuzzy: Option<bool>,  // if true, this property has FuzzyBoolean value, otherwise just true or false
}

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompositeDescriptorBuilder {
    pub properties: Option<PropertyMapBuilder>,
}

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegerDescriptorBuilder {
    pub format: Option<IntegerFormat>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
}
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IntegerFormat {
    I8(),
    I16(),
    I32(),
    I64(),
    // I128(),
    U8(),
    U16(),
    U32(),
    U64(),
    // U128(),
}

#[hdk_entry_helper]
#[derive(new, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StringDescriptorBuilder {
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    //pattern: Option<String>,
}


#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValueCollectionDescriptorBuilder {
    pub contains_items_of_type: Option<HolonReference>,
    pub min_items: Option<u32>,
    pub max_items: Option<u32>,
    pub unique_items: Option<bool>, // true means duplicate items are not allowed
    pub is_ordered: Option<bool>,   // if items have an intrinsic order
}
