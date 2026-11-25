mod configuration;
mod error;
mod render;
mod scalar;
mod syntax;

pub use error::{
    TemplateConfigurationDefect, TemplateRenderError, TemplateSyntaxError,
    TemplateSyntaxErrorReason,
};
pub use render::TemplateInput;
pub use syntax::PositionalStringTemplate;

#[cfg(test)]
mod tests;
