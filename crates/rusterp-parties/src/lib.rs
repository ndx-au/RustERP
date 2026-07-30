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
//! [`PartyRepository`] is implemented by [`InMemoryPartyRepository`] (tests) and
//! [`PostgresPartyRepository`] (production via sqlx pool).
//!
//! # Module activation
//!
//! Functional module id: [`MODULE_ID`] (`"parties"`). Not always-on.
//!
//! Suggested RBAC resources (not enforced here): `party:read`, `party:write`.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rusterp_modules::{ModuleDescriptor, ModuleId, ModuleRegistry};
use uuid::Uuid;

mod postgres;

pub use postgres::PostgresPartyRepository;

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
    pub active: bool,
}

/// Input for attaching a contact to a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewContact {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Patch for updating a contact.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContactUpdate {
    pub name: Option<String>,
    pub email: Option<Option<String>>,
    pub phone: Option<Option<String>>,
    pub active: Option<bool>,
}

/// Address kind for a party address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AddressKind {
    Billing,
    Shipping,
    Other,
}

/// Postal address belonging to a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub id: String,
    pub party_id: String,
    pub kind: AddressKind,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub active: bool,
}

/// Input for attaching an address to a party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAddress {
    pub kind: AddressKind,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

/// Patch for updating an address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddressUpdate {
    pub kind: Option<AddressKind>,
    pub line1: Option<String>,
    pub line2: Option<Option<String>>,
    pub city: Option<String>,
    pub state_region: Option<Option<String>>,
    pub postal_code: Option<Option<String>>,
    pub country: Option<String>,
    pub active: Option<bool>,
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

/// Persistence port for parties, contacts, and addresses.
#[async_trait]
pub trait PartyRepository: Send + Sync {
    async fn create_party(&self, new: NewParty) -> Result<Party, PartyError>;
    async fn get_party(&self, id: &str) -> Result<Party, PartyError>;
    /// When `role_filter` is `Some`, only parties holding that role are returned.
    async fn list_parties(
        &self,
        role_filter: Option<PartyRole>,
    ) -> Result<Vec<Party>, PartyError>;
    async fn update_party(&self, id: &str, update: PartyUpdate) -> Result<Party, PartyError>;

    async fn add_contact(&self, party_id: &str, new: NewContact) -> Result<Contact, PartyError>;
    async fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError>;
    async fn update_contact(&self, id: &str, update: ContactUpdate) -> Result<Contact, PartyError>;

    async fn add_address(&self, party_id: &str, new: NewAddress) -> Result<Address, PartyError>;
    async fn list_addresses(&self, party_id: &str) -> Result<Vec<Address>, PartyError>;
    async fn update_address(&self, id: &str, update: AddressUpdate) -> Result<Address, PartyError>;
}

/// In-memory [`PartyRepository`] for tests and early development.
#[derive(Debug, Default)]
pub struct InMemoryPartyRepository {
    parties: Mutex<HashMap<String, Party>>,
    contacts: Mutex<HashMap<String, Contact>>,
    addresses: Mutex<HashMap<String, Address>>,
}

impl InMemoryPartyRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PartyRepository for InMemoryPartyRepository {
    async fn create_party(&self, new: NewParty) -> Result<Party, PartyError> {
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
        self.parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .insert(party.id.clone(), party.clone());
        Ok(party)
    }

    async fn get_party(&self, id: &str) -> Result<Party, PartyError> {
        self.parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .get(id)
            .cloned()
            .ok_or_else(|| PartyError::NotFound {
                entity: "party",
                id: id.to_string(),
            })
    }

    async fn list_parties(
        &self,
        role_filter: Option<PartyRole>,
    ) -> Result<Vec<Party>, PartyError> {
        let mut list: Vec<_> = self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .values()
            .filter(|p| {
                role_filter
                    .map(|role| p.roles.contains(&role))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        list.sort_by(|a, b| a.display_name.cmp(&b.display_name).then(a.id.cmp(&b.id)));
        Ok(list)
    }

    async fn update_party(&self, id: &str, update: PartyUpdate) -> Result<Party, PartyError> {
        let mut parties = self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?;
        let party = parties
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

    async fn add_contact(&self, party_id: &str, new: NewContact) -> Result<Contact, PartyError> {
        if !self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .contains_key(party_id)
        {
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
            active: true,
        };
        self.contacts
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .insert(contact.id.clone(), contact.clone());
        Ok(contact)
    }

    async fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError> {
        if !self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .contains_key(party_id)
        {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }
        let mut list: Vec<_> = self
            .contacts
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .values()
            .filter(|c| c.party_id == party_id && c.active)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(list)
    }

    async fn update_contact(&self, id: &str, update: ContactUpdate) -> Result<Contact, PartyError> {
        let mut contacts = self
            .contacts
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?;
        let contact = contacts.get_mut(id).ok_or_else(|| PartyError::NotFound {
            entity: "contact",
            id: id.to_string(),
        })?;
        if let Some(name) = update.name {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(PartyError::Invalid("contact name must not be empty".into()));
            }
            contact.name = name;
        }
        if let Some(email) = update.email {
            contact.email = email;
        }
        if let Some(phone) = update.phone {
            contact.phone = phone;
        }
        if let Some(active) = update.active {
            contact.active = active;
        }
        Ok(contact.clone())
    }

    async fn add_address(&self, party_id: &str, new: NewAddress) -> Result<Address, PartyError> {
        if !self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .contains_key(party_id)
        {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }
        let line1 = new.line1.trim().to_string();
        let city = new.city.trim().to_string();
        let country = new.country.trim().to_uppercase();
        if line1.is_empty() || city.is_empty() {
            return Err(PartyError::Invalid(
                "address line1 and city must not be empty".into(),
            ));
        }
        if country.len() != 2 {
            return Err(PartyError::Invalid(
                "country must be ISO 3166-1 alpha-2".into(),
            ));
        }
        let address = Address {
            id: new_id(),
            party_id: party_id.to_string(),
            kind: new.kind,
            line1,
            line2: new.line2,
            city,
            state_region: new.state_region,
            postal_code: new.postal_code,
            country,
            active: true,
        };
        self.addresses
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .insert(address.id.clone(), address.clone());
        Ok(address)
    }

    async fn list_addresses(&self, party_id: &str) -> Result<Vec<Address>, PartyError> {
        if !self
            .parties
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .contains_key(party_id)
        {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }
        let mut list: Vec<_> = self
            .addresses
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?
            .values()
            .filter(|a| a.party_id == party_id && a.active)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.city.cmp(&b.city).then(a.id.cmp(&b.id)));
        Ok(list)
    }

    async fn update_address(&self, id: &str, update: AddressUpdate) -> Result<Address, PartyError> {
        let mut addresses = self
            .addresses
            .lock()
            .map_err(|e| PartyError::Invalid(format!("lock poisoned: {e}")))?;
        let address = addresses.get_mut(id).ok_or_else(|| PartyError::NotFound {
            entity: "address",
            id: id.to_string(),
        })?;
        if let Some(kind) = update.kind {
            address.kind = kind;
        }
        if let Some(line1) = update.line1 {
            let line1 = line1.trim().to_string();
            if line1.is_empty() {
                return Err(PartyError::Invalid(
                    "address line1 must not be empty".into(),
                ));
            }
            address.line1 = line1;
        }
        if let Some(line2) = update.line2 {
            address.line2 = line2;
        }
        if let Some(city) = update.city {
            let city = city.trim().to_string();
            if city.is_empty() {
                return Err(PartyError::Invalid("address city must not be empty".into()));
            }
            address.city = city;
        }
        if let Some(state_region) = update.state_region {
            address.state_region = state_region;
        }
        if let Some(postal_code) = update.postal_code {
            address.postal_code = postal_code;
        }
        if let Some(country) = update.country {
            let country = country.trim().to_uppercase();
            if country.len() != 2 {
                return Err(PartyError::Invalid(
                    "country must be ISO 3166-1 alpha-2".into(),
                ));
            }
            address.country = country;
        }
        if let Some(active) = update.active {
            address.active = active;
        }
        Ok(address.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(items: &[PartyRole]) -> BTreeSet<PartyRole> {
        items.iter().copied().collect()
    }

    #[tokio::test]
    async fn list_parties_filters_by_role() {
        let repo = InMemoryPartyRepository::new();
        let _ = repo
            .create_party(NewParty {
                display_name: "Cust".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .await
            .unwrap();
        let _ = repo
            .create_party(NewParty {
                display_name: "Supp".into(),
                roles: roles(&[PartyRole::Supplier]),
            })
            .await
            .unwrap();
        let filtered = repo
            .list_parties(Some(PartyRole::Customer))
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].display_name, "Cust");
    }

    #[tokio::test]
    async fn add_and_list_addresses_for_party() {
        let repo = InMemoryPartyRepository::new();
        let party = repo
            .create_party(NewParty {
                display_name: "With Address".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .await
            .unwrap();
        let addr = repo
            .add_address(
                &party.id,
                NewAddress {
                    kind: AddressKind::Billing,
                    line1: "10 Queen St".into(),
                    line2: None,
                    city: "Brisbane".into(),
                    state_region: Some("QLD".into()),
                    postal_code: Some("4000".into()),
                    country: "AU".into(),
                },
            )
            .await
            .unwrap();
        let list = repo.list_addresses(&party.id).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, addr.id);
        assert_eq!(list[0].kind, AddressKind::Billing);
    }

    #[tokio::test]
    async fn create_party_and_fetch_by_id() {
        let repo = InMemoryPartyRepository::new();
        let created = repo
            .create_party(NewParty {
                display_name: "Acme Ltd".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .await
            .expect("create");

        let fetched = repo.get_party(&created.id).await.expect("get");
        assert_eq!(fetched.display_name, "Acme Ltd");
        assert!(fetched.roles.contains(&PartyRole::Customer));
        assert!(fetched.active);
        assert!(!fetched.id.is_empty());
    }

    #[tokio::test]
    async fn party_can_be_customer_and_supplier() {
        let repo = InMemoryPartyRepository::new();
        let party = repo
            .create_party(NewParty {
                display_name: "Dual Role Co".into(),
                roles: roles(&[PartyRole::Customer, PartyRole::Supplier]),
            })
            .await
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
            .await
            .expect("update");
        assert_eq!(updated.roles.len(), 3);
    }

    #[tokio::test]
    async fn add_and_list_contacts_for_party() {
        let repo = InMemoryPartyRepository::new();
        let party = repo
            .create_party(NewParty {
                display_name: "With Contacts".into(),
                roles: roles(&[PartyRole::Prospect]),
            })
            .await
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
            .await
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
            .await
            .expect("contact 2");

        let list = repo.list_contacts(&party.id).await.expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Ada Lovelace");
        assert_eq!(list[0].id, c1.id);
        assert_eq!(list[0].party_id, party.id);
    }

    #[tokio::test]
    async fn unknown_party_id_returns_not_found() {
        let repo = InMemoryPartyRepository::new();
        let err = repo
            .get_party("does-not-exist")
            .await
            .expect_err("missing");
        assert_eq!(
            err,
            PartyError::NotFound {
                entity: "party",
                id: "does-not-exist".into(),
            }
        );

        let err = repo
            .add_contact(
                "missing-party",
                NewContact {
                    name: "Nobody".into(),
                    email: None,
                    phone: None,
                },
            )
            .await
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
