//! Parties functional domain for RustERP.
//!
//! A **Party** is a single aggregate that may hold one or more business roles
//! (`Customer`, `Supplier`, `Prospect`). Contacts belong to a party.
//!
//! # Identifiers
//!
//! Entity ids are **UUID v4** values encoded as hyphenated lowercase strings
//! (generated in-process via the `uuid` crate).
//!
//! # Persistence
//!
//! Phase 1 provides [`PartyRepository`] + [`InMemoryPartyRepository`] only.
//! No SQL drivers and no dependency on `rusterp-storage`.
//!
//! # Module activation
//!
//! Functional module id: [`MODULE_ID`] (`"parties"`). Not always-on.
//!
//! Suggested RBAC resources (not enforced here): `party:read`, `party:write`.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use rusterp_modules::{ModuleDescriptor, ModuleId, ModuleRegistry};
use uuid::Uuid;

/// Functional domain module id registered with `rusterp-modules`.
pub const MODULE_ID: &str = "parties";

/// Human-readable module name for the registry.
pub const MODULE_NAME: &str = "Parties";

/// Register the Parties functional domain on a module registry (disabled by default).
pub fn register_module(registry: &mut ModuleRegistry) {
    registry.register(ModuleDescriptor::new(MODULE_ID, MODULE_NAME));
}

/// Enable the Parties module after it has been registered.
pub fn enable_module(registry: &mut ModuleRegistry) -> Result<(), rusterp_modules::ModuleError> {
    registry.enable(&ModuleId::new(MODULE_ID))
}

/// Generate a new opaque entity id (UUID v4 string).
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Business role a party may hold. A party may hold several at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PartyRole {
    Customer,
    Supplier,
    Prospect,
}

/// A party (customer / supplier / prospect — or any combination).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    pub id: String,
    pub display_name: String,
    pub roles: BTreeSet<PartyRole>,
    /// Unix timestamp (seconds) when the party record was created.
    pub created_at: u64,
    /// When false, the party is archived (still stored; not a hard delete).
    pub active: bool,
}

/// Input for creating a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewParty {
    pub display_name: String,
    pub roles: BTreeSet<PartyRole>,
}

/// Patch for updating a party. `None` fields are left unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartyUpdate {
    pub display_name: Option<String>,
    pub roles: Option<BTreeSet<PartyRole>>,
    pub active: Option<bool>,
}

/// Contact belonging to a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub id: String,
    pub party_id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Input for attaching a contact to a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewContact {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Repository / domain errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyError {
    NotFound { entity: &'static str, id: String },
    Invalid(String),
}

impl fmt::Display for PartyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartyError::NotFound { entity, id } => {
                write!(f, "{entity} not found: {id}")
            }
            PartyError::Invalid(msg) => write!(f, "invalid party data: {msg}"),
        }
    }
}

impl std::error::Error for PartyError {}

/// Persistence port for parties and contacts.
pub trait PartyRepository: Send + Sync {
    fn create_party(&mut self, new: NewParty) -> Result<Party, PartyError>;
    fn get_party(&self, id: &str) -> Result<Party, PartyError>;
    fn list_parties(&self) -> Vec<Party>;
    fn update_party(&mut self, id: &str, update: PartyUpdate) -> Result<Party, PartyError>;

    fn add_contact(&mut self, party_id: &str, new: NewContact) -> Result<Contact, PartyError>;
    fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError>;
}

/// In-memory [`PartyRepository`] for tests and early development.
#[derive(Debug, Default)]
pub struct InMemoryPartyRepository {
    parties: HashMap<String, Party>,
    contacts: HashMap<String, Contact>,
}

impl InMemoryPartyRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PartyRepository for InMemoryPartyRepository {
    fn create_party(&mut self, new: NewParty) -> Result<Party, PartyError> {
        let display_name = new.display_name.trim().to_string();
        if display_name.is_empty() {
            return Err(PartyError::Invalid(
                "display_name must not be empty".into(),
            ));
        }
        if new.roles.is_empty() {
            return Err(PartyError::Invalid(
                "party must have at least one role".into(),
            ));
        }

        let party = Party {
            id: new_id(),
            display_name,
            roles: new.roles,
            created_at: now_unix_secs(),
            active: true,
        };
        self.parties.insert(party.id.clone(), party.clone());
        Ok(party)
    }

    fn get_party(&self, id: &str) -> Result<Party, PartyError> {
        self.parties
            .get(id)
            .cloned()
            .ok_or_else(|| PartyError::NotFound {
                entity: "party",
                id: id.to_string(),
            })
    }

    fn list_parties(&self) -> Vec<Party> {
        let mut list: Vec<_> = self.parties.values().cloned().collect();
        list.sort_by(|a, b| a.display_name.cmp(&b.display_name).then(a.id.cmp(&b.id)));
        list
    }

    fn update_party(&mut self, id: &str, update: PartyUpdate) -> Result<Party, PartyError> {
        let party = self
            .parties
            .get_mut(id)
            .ok_or_else(|| PartyError::NotFound {
                entity: "party",
                id: id.to_string(),
            })?;

        if let Some(name) = update.display_name {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(PartyError::Invalid(
                    "display_name must not be empty".into(),
                ));
            }
            party.display_name = name;
        }
        if let Some(roles) = update.roles {
            if roles.is_empty() {
                return Err(PartyError::Invalid(
                    "party must have at least one role".into(),
                ));
            }
            party.roles = roles;
        }
        if let Some(active) = update.active {
            party.active = active;
        }

        Ok(party.clone())
    }

    fn add_contact(&mut self, party_id: &str, new: NewContact) -> Result<Contact, PartyError> {
        if !self.parties.contains_key(party_id) {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }
        let name = new.name.trim().to_string();
        if name.is_empty() {
            return Err(PartyError::Invalid("contact name must not be empty".into()));
        }

        let contact = Contact {
            id: new_id(),
            party_id: party_id.to_string(),
            name,
            email: new.email,
            phone: new.phone,
        };
        self.contacts.insert(contact.id.clone(), contact.clone());
        Ok(contact)
    }

    fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError> {
        if !self.parties.contains_key(party_id) {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }
        let mut list: Vec<_> = self
            .contacts
            .values()
            .filter(|c| c.party_id == party_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(items: &[PartyRole]) -> BTreeSet<PartyRole> {
        items.iter().copied().collect()
    }

    #[test]
    fn create_party_and_fetch_by_id() {
        let mut repo = InMemoryPartyRepository::new();
        let created = repo
            .create_party(NewParty {
                display_name: "Acme Ltd".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .expect("create");

        let fetched = repo.get_party(&created.id).expect("get");
        assert_eq!(fetched.display_name, "Acme Ltd");
        assert!(fetched.roles.contains(&PartyRole::Customer));
        assert!(fetched.active);
        assert!(!fetched.id.is_empty());
    }

    #[test]
    fn party_can_be_customer_and_supplier() {
        let mut repo = InMemoryPartyRepository::new();
        let party = repo
            .create_party(NewParty {
                display_name: "Dual Role Co".into(),
                roles: roles(&[PartyRole::Customer, PartyRole::Supplier]),
            })
            .expect("create");

        assert!(party.roles.contains(&PartyRole::Customer));
        assert!(party.roles.contains(&PartyRole::Supplier));
        assert_eq!(party.roles.len(), 2);

        let updated = repo
            .update_party(
                &party.id,
                PartyUpdate {
                    roles: Some(roles(&[
                        PartyRole::Customer,
                        PartyRole::Supplier,
                        PartyRole::Prospect,
                    ])),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.roles.len(), 3);
    }

    #[test]
    fn add_and_list_contacts_for_party() {
        let mut repo = InMemoryPartyRepository::new();
        let party = repo
            .create_party(NewParty {
                display_name: "With Contacts".into(),
                roles: roles(&[PartyRole::Prospect]),
            })
            .expect("create");

        let c1 = repo
            .add_contact(
                &party.id,
                NewContact {
                    name: "Ada Lovelace".into(),
                    email: Some("ada@example.com".into()),
                    phone: None,
                },
            )
            .expect("contact 1");
        let _c2 = repo
            .add_contact(
                &party.id,
                NewContact {
                    name: "Grace Hopper".into(),
                    email: None,
                    phone: Some("+1-555-0100".into()),
                },
            )
            .expect("contact 2");

        let list = repo.list_contacts(&party.id).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Ada Lovelace");
        assert_eq!(list[0].id, c1.id);
        assert_eq!(list[0].party_id, party.id);
    }

    #[test]
    fn unknown_party_id_returns_not_found() {
        let repo = InMemoryPartyRepository::new();
        let err = repo.get_party("does-not-exist").expect_err("missing");
        assert_eq!(
            err,
            PartyError::NotFound {
                entity: "party",
                id: "does-not-exist".into(),
            }
        );

        let mut repo = InMemoryPartyRepository::new();
        let err = repo
            .add_contact(
                "missing-party",
                NewContact {
                    name: "Nobody".into(),
                    email: None,
                    phone: None,
                },
            )
            .expect_err("missing party");
        assert!(matches!(err, PartyError::NotFound { entity: "party", .. }));
    }

    #[test]
    fn register_and_enable_parties_module() {
        let mut registry = ModuleRegistry::new();
        register_module(&mut registry);

        let id = ModuleId::new(MODULE_ID);
        assert_eq!(MODULE_ID, "parties");
        assert!(registry.is_registered(&id));
        assert!(!registry.is_enabled(&id));

        enable_module(&mut registry).expect("enable parties");
        assert!(registry.is_enabled(&id));
    }
}
