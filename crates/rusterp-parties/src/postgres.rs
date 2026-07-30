//! PostgreSQL-backed [`PartyRepository`] (schema `party`).

use std::collections::BTreeSet;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    new_id, Address, AddressKind, AddressUpdate, Contact, ContactUpdate, NewAddress, NewContact,
    NewParty, Party, PartyError, PartyRepository, PartyRole, PartyUpdate,
};

/// PostgreSQL-backed [`PartyRepository`].
pub struct PostgresPartyRepository {
    pool: PgPool,
}

impl PostgresPartyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_uuid(id: &str) -> Result<Uuid, PartyError> {
        Uuid::parse_str(id).map_err(|_| PartyError::Invalid(format!("invalid UUID: {id}")))
    }

    fn role_from_db(s: &str) -> Option<PartyRole> {
        match s {
            "customer" => Some(PartyRole::Customer),
            "supplier" => Some(PartyRole::Supplier),
            "prospect" => Some(PartyRole::Prospect),
            _ => None,
        }
    }

    fn role_to_db(role: &PartyRole) -> &'static str {
        match role {
            PartyRole::Customer => "customer",
            PartyRole::Supplier => "supplier",
            PartyRole::Prospect => "prospect",
        }
    }

    fn kind_from_db(s: &str) -> AddressKind {
        match s {
            "billing" => AddressKind::Billing,
            "shipping" => AddressKind::Shipping,
            _ => AddressKind::Other,
        }
    }

    fn kind_to_db(kind: &AddressKind) -> &'static str {
        match kind {
            AddressKind::Billing => "billing",
            AddressKind::Shipping => "shipping",
            AddressKind::Other => "other",
        }
    }

    async fn load_roles(&self, party_id: &str) -> Result<BTreeSet<PartyRole>, PartyError> {
        let role_rows = sqlx::query_scalar::<_, String>(
            "SELECT role::text FROM party.party_roles WHERE party_id = $1::uuid",
        )
        .bind(party_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("query roles: {e}")))?;

        Ok(role_rows
            .into_iter()
            .filter_map(|s| Self::role_from_db(&s))
            .collect())
    }
}

#[async_trait]
impl PartyRepository for PostgresPartyRepository {
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

        let id = new_id();
        Self::parse_uuid(&id)?;

        let mut txn = self
            .pool
            .begin()
            .await
            .map_err(|e| PartyError::Invalid(format!("postgres transaction: {e}")))?;

        let row = sqlx::query_as::<_, (String, String, i64, bool)>(
            "INSERT INTO party.parties (id, display_name) VALUES ($1::uuid, $2)
             RETURNING id::text, display_name, EXTRACT(EPOCH FROM created_at)::bigint, active",
        )
        .bind(&id)
        .bind(&display_name)
        .fetch_one(&mut *txn)
        .await
        .map_err(|e| PartyError::Invalid(format!("insert party: {e}")))?;

        for role in &new.roles {
            sqlx::query(
                "INSERT INTO party.party_roles (party_id, role) VALUES ($1::uuid, $2::party.party_role)",
            )
            .bind(&id)
            .bind(Self::role_to_db(role))
            .execute(&mut *txn)
            .await
            .map_err(|e| PartyError::Invalid(format!("insert role: {e}")))?;
        }

        txn.commit()
            .await
            .map_err(|e| PartyError::Invalid(format!("commit create_party: {e}")))?;

        Ok(Party {
            id: row.0,
            display_name: row.1,
            roles: new.roles,
            created_at: row.2 as u64,
            active: row.3,
        })
    }

    async fn get_party(&self, id: &str) -> Result<Party, PartyError> {
        Self::parse_uuid(id)?;

        let row = sqlx::query_as::<_, (String, String, i64, bool)>(
            "SELECT id::text, display_name, EXTRACT(EPOCH FROM created_at)::bigint, active
             FROM party.parties WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("query party: {e}")))?
        .ok_or_else(|| PartyError::NotFound {
            entity: "party",
            id: id.to_string(),
        })?;

        let roles = self.load_roles(id).await?;

        Ok(Party {
            id: row.0,
            display_name: row.1,
            roles,
            created_at: row.2 as u64,
            active: row.3,
        })
    }

    async fn list_parties(
        &self,
        role_filter: Option<PartyRole>,
    ) -> Result<Vec<Party>, PartyError> {
        let rows = if let Some(role) = role_filter {
            sqlx::query_as::<_, (String, String, i64, bool)>(
                "SELECT p.id::text, p.display_name, EXTRACT(EPOCH FROM p.created_at)::bigint, p.active
                 FROM party.parties p
                 INNER JOIN party.party_roles r ON r.party_id = p.id
                 WHERE r.role = $1::party.party_role
                 ORDER BY p.display_name, p.id",
            )
            .bind(Self::role_to_db(&role))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, (String, String, i64, bool)>(
                "SELECT id::text, display_name, EXTRACT(EPOCH FROM created_at)::bigint, active
                 FROM party.parties ORDER BY display_name, id",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| PartyError::Invalid(format!("list parties: {e}")))?;

        let mut parties = Vec::with_capacity(rows.len());
        for row in rows {
            let roles = self.load_roles(&row.0).await?;
            parties.push(Party {
                id: row.0,
                display_name: row.1,
                roles,
                created_at: row.2 as u64,
                active: row.3,
            });
        }
        Ok(parties)
    }

    async fn update_party(&self, id: &str, update: PartyUpdate) -> Result<Party, PartyError> {
        Self::parse_uuid(id)?;

        let current = sqlx::query_as::<_, (String, bool)>(
            "SELECT display_name, active FROM party.parties WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("fetch party for update: {e}")))?
        .ok_or_else(|| PartyError::NotFound {
            entity: "party",
            id: id.to_string(),
        })?;

        let new_name = update
            .display_name
            .as_deref()
            .unwrap_or(&current.0)
            .trim()
            .to_string();
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

        let active = update.active.unwrap_or(current.1);

        sqlx::query(
            "UPDATE party.parties SET display_name = $1, active = $2, row_version = row_version + 1
             WHERE id = $3::uuid",
        )
        .bind(&new_name)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("update party: {e}")))?;

        if let Some(ref roles) = update.roles {
            sqlx::query("DELETE FROM party.party_roles WHERE party_id = $1::uuid")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| PartyError::Invalid(format!("delete roles for update: {e}")))?;

            for role in roles {
                sqlx::query(
                    "INSERT INTO party.party_roles (party_id, role) VALUES ($1::uuid, $2::party.party_role)",
                )
                .bind(id)
                .bind(Self::role_to_db(role))
                .execute(&self.pool)
                .await
                .map_err(|e| PartyError::Invalid(format!("insert role for update: {e}")))?;
            }
        }

        self.get_party(id).await
    }

    async fn add_contact(&self, party_id: &str, new: NewContact) -> Result<Contact, PartyError> {
        Self::parse_uuid(party_id)?;

        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id::text FROM party.parties WHERE id = $1::uuid",
        )
        .bind(party_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("check party exists: {e}")))?;

        if exists.is_none() {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }

        let name = new.name.trim().to_string();
        if name.is_empty() {
            return Err(PartyError::Invalid("contact name must not be empty".into()));
        }

        let contact_id = new_id();
        Self::parse_uuid(&contact_id)?;

        sqlx::query(
            "INSERT INTO party.contacts (id, party_id, name, email, phone)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5)",
        )
        .bind(&contact_id)
        .bind(party_id)
        .bind(&name)
        .bind(&new.email)
        .bind(&new.phone)
        .execute(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("insert contact: {e}")))?;

        Ok(Contact {
            id: contact_id,
            party_id: party_id.to_string(),
            name,
            email: new.email,
            phone: new.phone,
            active: true,
        })
    }

    async fn list_contacts(&self, party_id: &str) -> Result<Vec<Contact>, PartyError> {
        Self::parse_uuid(party_id)?;

        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id::text FROM party.parties WHERE id = $1::uuid",
        )
        .bind(party_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("check party exists: {e}")))?;

        if exists.is_none() {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }

        let contacts = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, bool)>(
            "SELECT id::text, party_id::text, name, email::text, phone, active FROM party.contacts
             WHERE party_id = $1::uuid AND active = TRUE ORDER BY name, id",
        )
        .bind(party_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("list contacts: {e}")))?;

        Ok(contacts
            .into_iter()
            .map(|row| Contact {
                id: row.0,
                party_id: row.1,
                name: row.2,
                email: row.3,
                phone: row.4,
                active: row.5,
            })
            .collect())
    }

    async fn update_contact(&self, id: &str, update: ContactUpdate) -> Result<Contact, PartyError> {
        Self::parse_uuid(id)?;
        let current = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, bool)>(
            "SELECT id::text, party_id::text, name, email::text, phone, active FROM party.contacts
             WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("get contact: {e}")))?
        .ok_or_else(|| PartyError::NotFound {
            entity: "contact",
            id: id.to_string(),
        })?;

        let name = match update.name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(PartyError::Invalid("contact name must not be empty".into()));
                }
                n
            }
            None => current.2,
        };
        let email = update.email.unwrap_or(current.3);
        let phone = update.phone.unwrap_or(current.4);
        let active = update.active.unwrap_or(current.5);

        sqlx::query(
            "UPDATE party.contacts SET name = $1, email = $2, phone = $3, active = $4,
             row_version = row_version + 1 WHERE id = $5::uuid",
        )
        .bind(&name)
        .bind(&email)
        .bind(&phone)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("update contact: {e}")))?;

        Ok(Contact {
            id: current.0,
            party_id: current.1,
            name,
            email,
            phone,
            active,
        })
    }

    async fn add_address(&self, party_id: &str, new: NewAddress) -> Result<Address, PartyError> {
        Self::parse_uuid(party_id)?;

        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id::text FROM party.parties WHERE id = $1::uuid",
        )
        .bind(party_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("check party exists: {e}")))?;

        if exists.is_none() {
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

        let address_id = new_id();
        Self::parse_uuid(&address_id)?;

        sqlx::query(
            "INSERT INTO party.addresses
             (id, party_id, kind, line1, line2, city, state_region, postal_code, country)
             VALUES ($1::uuid, $2::uuid, $3::party.address_kind, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&address_id)
        .bind(party_id)
        .bind(Self::kind_to_db(&new.kind))
        .bind(&line1)
        .bind(&new.line2)
        .bind(&city)
        .bind(&new.state_region)
        .bind(&new.postal_code)
        .bind(&country)
        .execute(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("insert address: {e}")))?;

        Ok(Address {
            id: address_id,
            party_id: party_id.to_string(),
            kind: new.kind,
            line1,
            line2: new.line2,
            city,
            state_region: new.state_region,
            postal_code: new.postal_code,
            country,
            active: true,
        })
    }

    async fn list_addresses(&self, party_id: &str) -> Result<Vec<Address>, PartyError> {
        Self::parse_uuid(party_id)?;

        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id::text FROM party.parties WHERE id = $1::uuid",
        )
        .bind(party_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("check party exists: {e}")))?;

        if exists.is_none() {
            return Err(PartyError::NotFound {
                entity: "party",
                id: party_id.to_string(),
            });
        }

        let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Option<String>, Option<String>, String, bool)>(
            "SELECT id::text, party_id::text, kind::text, line1, line2, city, state_region, postal_code, country, active
             FROM party.addresses
             WHERE party_id = $1::uuid AND active = TRUE
             ORDER BY city, id",
        )
        .bind(party_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("list addresses: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|row| Address {
                id: row.0,
                party_id: row.1,
                kind: Self::kind_from_db(&row.2),
                line1: row.3,
                line2: row.4,
                city: row.5,
                state_region: row.6,
                postal_code: row.7,
                country: row.8,
                active: row.9,
            })
            .collect())
    }

    async fn update_address(&self, id: &str, update: AddressUpdate) -> Result<Address, PartyError> {
        Self::parse_uuid(id)?;
        let current = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Option<String>, Option<String>, String, bool)>(
            "SELECT id::text, party_id::text, kind::text, line1, line2, city, state_region, postal_code, country, active
             FROM party.addresses WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("get address: {e}")))?
        .ok_or_else(|| PartyError::NotFound {
            entity: "address",
            id: id.to_string(),
        })?;

        let kind = update.kind.unwrap_or_else(|| Self::kind_from_db(&current.2));
        let line1 = match update.line1 {
            Some(l) => {
                let l = l.trim().to_string();
                if l.is_empty() {
                    return Err(PartyError::Invalid(
                        "address line1 must not be empty".into(),
                    ));
                }
                l
            }
            None => current.3,
        };
        let line2 = update.line2.unwrap_or(current.4);
        let city = match update.city {
            Some(c) => {
                let c = c.trim().to_string();
                if c.is_empty() {
                    return Err(PartyError::Invalid("address city must not be empty".into()));
                }
                c
            }
            None => current.5,
        };
        let state_region = update.state_region.unwrap_or(current.6);
        let postal_code = update.postal_code.unwrap_or(current.7);
        let country = match update.country {
            Some(c) => {
                let c = c.trim().to_uppercase();
                if c.len() != 2 {
                    return Err(PartyError::Invalid(
                        "country must be ISO 3166-1 alpha-2".into(),
                    ));
                }
                c
            }
            None => current.8,
        };
        let active = update.active.unwrap_or(current.9);

        sqlx::query(
            "UPDATE party.addresses SET kind = $1::party.address_kind, line1 = $2, line2 = $3,
             city = $4, state_region = $5, postal_code = $6, country = $7, active = $8,
             row_version = row_version + 1 WHERE id = $9::uuid",
        )
        .bind(Self::kind_to_db(&kind))
        .bind(&line1)
        .bind(&line2)
        .bind(&city)
        .bind(&state_region)
        .bind(&postal_code)
        .bind(&country)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PartyError::Invalid(format!("update address: {e}")))?;

        Ok(Address {
            id: current.0,
            party_id: current.1,
            kind,
            line1,
            line2,
            city,
            state_region,
            postal_code,
            country,
            active,
        })
    }
}
