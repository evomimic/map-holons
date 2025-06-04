pub const ENUM_TEMPLATE: &str = r#"
// Auto-generated enum from {{source_file}}
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum {{enum_name}} {
    #[default]
    {{first_variant}},
    {{#each other_variants}}
    {{this}},
    {{/each}}
}

impl fmt::Display for {{enum_name}} {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            {{enum_name}}::{{first_variant}} => write!(f, "{{first_variant}}"),
            {{#each other_variants}}
            {{../enum_name}}::{{this}} => write!(f, "{{this}}"),
            {{/each}}
        }
    }
}

impl FromStr for {{enum_name}} {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "{{first_variant}}" => Ok({{enum_name}}::{{first_variant}}),
            {{#each other_variants}}
            "{{this}}" => Ok({{../enum_name}}::{{this}}),
            {{/each}}
            _ => Err(()),
        }
    }
}
"#;
