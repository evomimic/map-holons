mod error;
mod syntax;

pub use error::{TemplateSyntaxError, TemplateSyntaxErrorReason};
pub use syntax::PositionalStringTemplate;

#[cfg(test)]
mod tests;
