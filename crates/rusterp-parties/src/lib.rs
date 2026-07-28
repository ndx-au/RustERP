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
use std::sync::{Arc, Mutex};
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

// ---------------------------------------------------------------------------
// SQLite-backed repository
// ---------------------------------------------------------------------------

/// SQLite-backed [`PartyRepository`].
///
/// Wraps a shared `Arc<Mutex<rusqlite::Connection>>` so that the domain
/// repo can live alongside the storage layer using the same database file.
pub struct SqlitePartyRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqlitePartyRepository {
    /// Create a new SQLite-backed repository from an existing connection handle.
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn role_from_str(s: &str) -> Option<PartyRole> {
        match s {
            "Customer" => Some(PartyRole::Customer),
            "Supplier" => Some(PartyRole::Supplier),
            "Prospect" => Some(PartyRole::Prospect),
            _ => None,
        }
    }

    fn role_to_str(role: &PartyRole) -> &'static str {
        match role {
            PartyRole::Customer => "Customer",
            PartyRole::Supplier => "Supplier",
            PartyRole::Prospect => "Prospect",
        }
    }
}

impl PartyRepository for SqlitePartyRepository {
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

        let id = new_id();
        let created_at = now_unix_secs();

        let mut guard = self.conn.lock().map_err(|e| {
            PartyError::Invalid(format!("sqlite lock poisoned: {e}"))
        })?;

        let txn = guard.transaction().map_err(|e| {
            PartyError::Invalid(format!("sqlite transaction: {e}"))
        })?;

        txn.execute(
            "INSERT INTO parties (id, display_name, created_at, active) VALUES (?, ?, ?, 1)",
            rusqlite::params![&id, &display_name, created_at as i64],
        )
        .map_err(|e| PartyError::Invalid(format!("insert party: {e}")))?;

        for role in &new.roles {
            txn.execute(
                "INSERT INTO party_roles (party_id, role) VALUES (?, ?)",
                rusqlite::params![&id, Self::role_to_str(role)],
            )
            .map_err(|e| PartyError::Invalid(format!("insert role: {e}")))?;
        }

        txn.commit().map_err(|e| {
            PartyError::Invalid(format!("commit create_party: {e}"))
        })?;

        drop(guard);

        Ok(Party {
            id,
            display_name,
            roles: new.roles,
            created_at,
            active: true,
        })
    }

    fn get_party(&self, id: &str) -> Result<Party, PartyError> {
        // First select the party row
        let row = {
            let guard = self
                .conn
                .lock()
                .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;

            guard
                .query_row(
                    "SELECT id, display_name, created_at, active FROM parties WHERE id = ?",
                    [&id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .map_err(|_| PartyError::NotFound {
                    entity: "party",
                    id: id.to_string(),
                })?
        };

        // Then select roles — force collect() into a binding so the
        // guard can be dropped before the result is used.
        let roles: BTreeSet<PartyRole> = {
            let guard = self
                .conn
                .lock()
                .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;

            let mut stmt = guard
                .prepare("SELECT role FROM party_roles WHERE party_id = ?")
                .map_err(|_| PartyError::NotFound {
                    entity: "party",
                    id: id.to_string(),
                })?;

            stmt.query_map([&row.0], |r| r.get::<_, String>(0))
                .ok()
                .map(|mrows| {
                    mrows.filter_map(|r| r.ok())
                        .filter_map(|s| Self::role_from_str(&s))
                        .collect()
                })
                .unwrap_or_default()
        };

        Ok(Party {
            id: row.0,
            display_name: row.1,
            roles,
            created_at: row.2 as u64,
            active: row.3 != 0,
        })
    }

    fn list_parties(&self) -> Vec<Party> {
        let rows: Vec<(String, String, i64, i64)> = {
            let guard = match self.conn.lock() {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };
            let mut stmt = match guard.prepare(
                "SELECT id, display_name, created_at, active FROM parties \
                 ORDER BY display_name, id",
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .ok()
            .map(|mut mrows| {
                let mut out = Vec::new();
                while let Some(r) = mrows.next() {
                    if let Ok(row) = r {
                        out.push(row);
                    }
                }
                out
            })
            .unwrap_or_default()
        };

        rows.into_iter()
            .filter_map(|row| {
                let roles: BTreeSet<PartyRole> = {
                    let guard = match self.conn.lock() {
                        Ok(g) => g,
                        Err(_) => return None,
                    };
                    let mut stmt = match guard.prepare(
                        "SELECT role FROM party_roles WHERE party_id = ?",
                    ) {
                        Ok(s) => s,
                        Err(_) => return None,
                    };
                    stmt.query_map([&row.0], |r| r.get::<_, String>(0))
                        .ok()
                        .map(|mrows| {
                            mrows.filter_map(|r| r.ok())
                                .filter_map(|s| Self::role_from_str(&s))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                Some(Party {
                    id: row.0,
                    display_name: row.1,
                    roles,
                    created_at: row.2 as u64,
                    active: row.3 != 0,
                })
            })
            .collect()
    }

    fn update_party(&mut self, id: &str, update: PartyUpdate) -> Result<Party, PartyError> {
        let guard = self
            .conn
            .lock()
            .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;

        // Fetch current state
        let current = guard
            .query_row(
                "SELECT id, display_name, active FROM parties WHERE id = ?",
                [&id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .map_err(|_| PartyError::NotFound {
                entity: "party",
                id: id.to_string(),
            })?;

        // Validate new values
        let new_name = update
            .display_name
            .as_deref()
            .unwrap_or(&current.1)
            .trim();
        if new_name.is_empty() {
            return Err(PartyError::Invalid("display_name must not be empty".into()));
        }
        if let Some(ref roles) = update.roles {
            if roles.is_empty() {
                return Err(PartyError::Invalid(
                    "party must have at least one role".into(),
                ));
            }
        }

        let active = update.active.unwrap_or(current.2 != 0);
        let active_val = if active { 1 } else { 0 };

        // Build update SQL
        guard
            .execute(
                "UPDATE parties SET display_name = ?, active = ? WHERE id = ?",
                rusqlite::params![new_name, active_val, &id],
            )
            .map_err(|e| PartyError::Invalid(format!("update party: {e}")))?;

        // Handle role updates
        if let Some(ref roles) = update.roles {
            guard
                .execute("DELETE FROM party_roles WHERE party_id = ?", [&id])
                .map_err(|e| {
                    PartyError::Invalid(format!("delete roles for update: {e}"))
                })?;
            for role in roles {
                guard
                    .execute(
                        "INSERT INTO party_roles (party_id, role) VALUES (?, ?)",
                        rusqlite::params![&id, Self::role_to_str(role)],
                    )
                    .map_err(|e| {
                        PartyError::Invalid(format!("insert role for update: {e}"))
                    })?;
            }
        }

        drop(guard);

        // Return the updated party by fetching it
        self.get_party(id)
    }

    fn add_contact(&mut self, party_id: &str, new: NewContact) -> Result<Contact, PartyError> {
        // Check party exists
        let exists = {
            let guard = self
                .conn
                .lock()
                .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;
            guard
                .query_row(
                    "SELECT id FROM parties WHERE id = ?",
                    [party_id],
                    |r| r.get::<_, String>(0),
                )
                .is_ok()
        };

        if !exists {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }

        let name = new.name.trim().to_string();
        if name.is_empty() {
            return Err(PartyError::Invalid("contact name must not be empty".into()));
        }

        let id = new_id();

        let mut guard = self
            .conn
            .lock()
            .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;

        let txn = guard.transaction().map_err(|e| {
            PartyError::Invalid(format!("sqlite transaction: {e}"))
        })?;

        txn.execute(
            "INSERT INTO contacts (id, party_id, name, email, phone) VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![&id, party_id, &name, new.email, new.phone],
        )
        .map_err(|e| PartyError::Invalid(format!("insert contact: {e}")))?;

        txn.commit().map_err(|e| {
            PartyError::Invalid(format!("commit add_contact: {e}"))
        })?;

        Ok(Contact {
            id,
            party_id: party_id.to_string(),
            name,
            email: new.email,
            phone: new.phone,
        })
    }

    fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError> {
        // Check party exists first
        {
            let guard = self
                .conn
                .lock()
                .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;
            let _ = guard
                .query_row(
                    "SELECT id FROM parties WHERE id = ?",
                    [party_id],
                    |r| r.get::<_, String>(0),
                )
                .map_err(|_| PartyError::NotFound {
                    entity: "party",
                    id: party_id.to_string(),
                })?;
        }

        let guard = self
            .conn
            .lock()
            .map_err(|e| PartyError::Invalid(format!("sqlite lock: {e}")))?;

        let mut stmt = guard
            .prepare(
                "SELECT id, party_id, name, email, phone FROM contacts \
                 WHERE party_id = ? ORDER BY name, id",
            )
            .map_err(|e| PartyError::Invalid(format!("prepare list_contacts: {e}")))?;

        let results = stmt
            .query_map([party_id], |r| {
                Ok(Contact {
                    id: r.get(0)?,
                    party_id: r.get(1)?,
                    name: r.get(2)?,
                    email: r.get(3)?,
                    phone: r.get(4)?,
                })
            })
            .map_err(|e| PartyError::Invalid(format!("query contacts: {e}")))?;

        let contacts: Result<Vec<_>, _> = results.collect();
        Ok(contacts.map_err(|e| PartyError::Invalid(format!("collect contacts: {e}")))?)
    }
}

// ---------------------------------------------------------------------------
// Tests — SqlitePartyRepository
// ---------------------------------------------------------------------------

#[cfg(test)]
mod sql_tests {
    use super::*;

    fn make_repo() -> SqlitePartyRepository {
        let conn = Arc::new(Mutex::new(
            rusqlite::Connection::open(":memory:").unwrap()
        ));
        let guard = conn.lock().unwrap();
        // Create the Parties domain tables directly — the migration
        // function lives in rusterp-storage, and parties crate keeps
        // its own dependencies light.
        guard.execute_batch(
            "CREATE TABLE IF NOT EXISTS parties (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS party_roles (
                party_id TEXT NOT NULL REFERENCES parties(id),
                role TEXT NOT NULL,
                PRIMARY KEY (party_id, role)
            );
            CREATE TABLE IF NOT EXISTS contacts (
                id TEXT PRIMARY KEY,
                party_id TEXT NOT NULL REFERENCES parties(id),
                name TEXT NOT NULL,
                email TEXT,
                phone TEXT
            );",
        )
        .unwrap();
        drop(guard);
        SqlitePartyRepository::new(conn)
    }

    fn roles(items: &[PartyRole]) -> BTreeSet<PartyRole> {
        items.iter().copied().collect()
    }

    #[test]
    fn create_party_persists_to_db() {
        let mut repo = make_repo();
        let created = repo
            .create_party(NewParty {
                display_name: "SQLite Party".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .expect("create");

        assert!(!created.id.is_empty());
        assert_eq!(created.display_name, "SQLite Party");
        assert!(created.roles.contains(&PartyRole::Customer));
        assert!(created.active);
        assert!(created.created_at > 0);
    }

    #[test]
    fn get_party_from_db() {
        let mut repo = make_repo();
        let created = repo
            .create_party(NewParty {
                display_name: "Fetch Me".into(),
                roles: roles(&[PartyRole::Supplier, PartyRole::Customer]),
            })
            .expect("create");

        let fetched = repo.get_party(&created.id).expect("get");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.display_name, "Fetch Me");
        assert!(fetched.roles.contains(&PartyRole::Supplier));
        assert!(fetched.roles.contains(&PartyRole::Customer));
    }

    #[test]
    fn list_parties_from_db() {
        let mut repo = make_repo();
        let _a = repo
            .create_party(NewParty {
                display_name: "Alpha".into(),
                roles: roles(&[PartyRole::Prospect]),
            })
            .expect("create a");
        let _b = repo
            .create_party(NewParty {
                display_name: "Beta".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .expect("create b");

        let list = repo.list_parties();
        assert_eq!(list.len(), 2);
        // Should be sorted by display_name
        assert_eq!(list[0].display_name, "Alpha");
        assert_eq!(list[1].display_name, "Beta");
    }

    #[test]
    fn update_party_changes() {
        let mut repo = make_repo();
        let created = repo
            .create_party(NewParty {
                display_name: "Original".into(),
                roles: roles(&[PartyRole::Prospect]),
            })
            .expect("create");

        let updated = repo
            .update_party(
                &created.id,
                PartyUpdate {
                    display_name: Some("Updated".into()),
                    roles: Some(roles(&[PartyRole::Customer, PartyRole::Supplier])),
                    active: Some(false),
                    ..Default::default()
                },
            )
            .expect("update");

        assert_eq!(updated.display_name, "Updated");
        assert_eq!(updated.roles.len(), 2);
        assert!(!updated.active);

        // Verify via fetch
        let fetched = repo.get_party(&created.id).expect("get after update");
        assert_eq!(fetched.display_name, "Updated");
        assert!(!fetched.active);
    }

    #[test]
    fn add_contact_to_party() {
        let mut repo = make_repo();
        let party = repo
            .create_party(NewParty {
                display_name: "Contact Party".into(),
                roles: roles(&[PartyRole::Customer]),
            })
            .expect("create");

        let contact = repo
            .add_contact(
                &party.id,
                NewContact {
                    name: "Ada".into(),
                    email: Some("ada@example.com".into()),
                    phone: None,
                },
            )
            .expect("add contact");

        assert_eq!(contact.party_id, party.id);
        assert_eq!(contact.name, "Ada");
        assert_eq!(contact.email, Some("ada@example.com".into()));
    }

    #[test]
    fn list_contacts() {
        let mut repo = make_repo();
        let party = repo
            .create_party(NewParty {
                display_name: "Contacts List".into(),
                roles: roles(&[PartyRole::Supplier]),
            })
            .expect("create");

        repo.add_contact(
            &party.id,
            NewContact {
                name: "Grace".into(),
                email: None,
                phone: None,
            },
        )
        .expect("contact 1");
        repo.add_contact(
            &party.id,
            NewContact {
                name: "Alan".into(),
                email: Some("alan@example.com".into()),
                phone: Some("+1-555".into()),
            },
        )
        .expect("contact 2");

        let list = repo.list_contacts(&party.id).expect("list");
        assert_eq!(list.len(), 2);
        // Sorted by name
        assert_eq!(list[0].name, "Alan");
        assert_eq!(list[1].name, "Grace");
    }

    #[test]
    fn unknown_party_id_returns_not_found() {
        let repo = make_repo();
        let err = repo
            .get_party("nonexistent")
            .expect_err("should fail");
        assert!(matches!(err, PartyError::NotFound { entity: "party", .. }));
    }

    #[test]
    fn contact_not_found_party_returns_error() {
        let mut repo = make_repo();
        let err = repo
            .add_contact(
                "no-such-party",
                NewContact {
                    name: "Nobody".into(),
                    email: None,
                    phone: None,
                },
            )
            .expect_err("should fail");
        assert!(matches!(err, PartyError::NotFound { entity: "party", .. }));
    }
}
