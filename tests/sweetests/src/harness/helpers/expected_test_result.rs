use core_types::HolonError;

#[derive(Debug, Clone)]
pub enum ExpectedTestResult {
    Success,
    Failure(HolonError),
}

impl ExpectedTestResult {
    pub fn is_ok(&self) -> bool {
        match self {
            Self::Success => true,
            Self::Failure(_) => false,
        }
    }
}
