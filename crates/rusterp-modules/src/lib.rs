//! Functional module registry for RustERP.
//!
//! **Functional domains** are user-facing capabilities consultants enable per business
//! (e.g. Sales, Inventory). They are distinct from **technical modules** (crates such as
//! `rusterp-storage`) that organize code but are not toggled by end users.
//!
//! This crate is a feature-flag style activation skeleton only.

use std::collections::HashMap;

/// Identifier for a functional domain module (stable string key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(String);

impl ModuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Metadata for a registerable functional module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDescriptor {
    pub id: ModuleId,
    pub name: String,
    /// When true, the module cannot be disabled (e.g. core platform).
    pub always_on: bool,
}

impl ModuleDescriptor {
    pub fn new(id: impl Into<ModuleId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            always_on: false,
        }
    }

    pub fn always_on(mut self) -> Self {
        self.always_on = true;
        self
    }
}

/// In-memory registry of functional modules and their activation state.
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    modules: HashMap<ModuleId, ModuleDescriptor>,
    enabled: HashMap<ModuleId, bool>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a module. New modules default to **disabled** unless `always_on`.
    pub fn register(&mut self, descriptor: ModuleDescriptor) {
        let enabled = descriptor.always_on;
        self.enabled.insert(descriptor.id.clone(), enabled);
        self.modules.insert(descriptor.id.clone(), descriptor);
    }

    /// Enable a registered module. Always-on modules stay enabled.
    pub fn enable(&mut self, id: &ModuleId) -> Result<(), ModuleError> {
        self.ensure_registered(id)?;
        self.enabled.insert(id.clone(), true);
        Ok(())
    }

    /// Disable a registered module. Fails for always-on modules.
    pub fn disable(&mut self, id: &ModuleId) -> Result<(), ModuleError> {
        let desc = self.ensure_registered(id)?;
        if desc.always_on {
            return Err(ModuleError::AlwaysOn {
                id: id.as_str().to_string(),
            });
        }
        self.enabled.insert(id.clone(), false);
        Ok(())
    }

    pub fn is_enabled(&self, id: &ModuleId) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }

    pub fn is_registered(&self, id: &ModuleId) -> bool {
        self.modules.contains_key(id)
    }

    pub fn descriptor(&self, id: &ModuleId) -> Option<&ModuleDescriptor> {
        self.modules.get(id)
    }

    fn ensure_registered(&self, id: &ModuleId) -> Result<&ModuleDescriptor, ModuleError> {
        self.modules
            .get(id)
            .ok_or_else(|| ModuleError::NotRegistered {
                id: id.as_str().to_string(),
            })
    }
}

/// Registry errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    NotRegistered { id: String },
    AlwaysOn { id: String },
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::NotRegistered { id } => write!(f, "module not registered: {id}"),
            ModuleError::AlwaysOn { id } => write!(f, "module is always-on and cannot be disabled: {id}"),
        }
    }
}

impl std::error::Error for ModuleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_enable_disable_inventory_style_toggle() {
        let mut reg = ModuleRegistry::new();
        reg.register(ModuleDescriptor::new("core", "Core Platform").always_on());
        reg.register(ModuleDescriptor::new("inventory", "Inventory"));

        let core = ModuleId::new("core");
        let inventory = ModuleId::new("inventory");

        assert!(reg.is_enabled(&core));
        assert!(!reg.is_enabled(&inventory));

        reg.enable(&inventory).expect("enable inventory");
        assert!(reg.is_enabled(&inventory));

        reg.disable(&inventory).expect("disable inventory");
        assert!(!reg.is_enabled(&inventory));

        let err = reg.disable(&core).expect_err("core is always-on");
        assert_eq!(
            err,
            ModuleError::AlwaysOn {
                id: "core".into()
            }
        );
    }
}
