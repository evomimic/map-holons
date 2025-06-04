use std::path::Path;

pub trait ParseTypeKind: Sized {
    fn type_kind_name() -> &'static str;
    fn parse_yaml(path: &Path) -> Result<Self, String>;
}
