//! Assessment-scoped structural evidence shared by exact definition identity.
use core_types::HolonError;
use holons_core::{
    core_shared_objects::transactions::TransactionContext, DefinitionIdentity,
    ExtendsLineageDiagnosis, HolonReference,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::Arc,
};

/// Memoizes diagnoses, including required-read failures, for one unchanged assessment.
/// It owns no descriptor acquisition or validation acceptance policy.
pub(crate) struct StructuralDiagnoses {
    context: Arc<TransactionContext>,
    sealed: Cell<bool>,
    entries: RefCell<HashMap<DefinitionIdentity, Result<Arc<ExtendsLineageDiagnosis>, HolonError>>>,
}
impl StructuralDiagnoses {
    /// Opens a structural evidence scope for the active transaction.
    pub fn new(context: &Arc<TransactionContext>) -> Self {
        Self {
            context: Arc::clone(context),
            sealed: Cell::new(false),
            entries: RefCell::new(HashMap::new()),
        }
    }
    /// Prevents evaluation from silently discovering undeclared structural demand.
    pub fn seal(&self) {
        self.sealed.set(true);
    }
    /// Resolves the exact definition identity without acquiring a descriptor product.
    pub fn identity(&self, reference: &HolonReference) -> Result<DefinitionIdentity, HolonError> {
        DefinitionIdentity::for_reference(reference, &self.context)
    }
    /// Looks up exact structural evidence without triggering diagnosis.
    pub fn existing(
        &self,
        reference: &HolonReference,
    ) -> Result<Option<Arc<ExtendsLineageDiagnosis>>, HolonError> {
        let identity = self.identity(reference)?;
        self.entries.borrow().get(&identity).cloned().transpose()
    }
    /// Shares the first diagnosis or required-read error for this exact definition.
    pub fn get(
        &self,
        reference: &HolonReference,
    ) -> Result<Arc<ExtendsLineageDiagnosis>, HolonError> {
        let identity = DefinitionIdentity::for_reference(reference, &self.context)?;
        if let Some(diagnosis) = self.entries.borrow().get(&identity) {
            return diagnosis.clone();
        }
        if self.sealed.get() {
            return Err(HolonError::InvalidState(format!(
                "Missing structural diagnosis for {} (assessment consumer)",
                reference.reference_id_string()
            )));
        }
        let definition = identity.definition_reference(&self.context, reference);
        let diagnosis = ExtendsLineageDiagnosis::assess(&definition).map(Arc::new);
        self.entries.borrow_mut().insert(identity, diagnosis.clone());
        diagnosis
    }
}
