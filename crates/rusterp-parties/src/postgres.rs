//! PostgreSQL-backed [`PartyRepository`] (schema `party`).

use std::collections::BTreeSet;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    new_id, Contact, NewContact, NewParty, Party, PartyError, PartyRepository, PartyRole,
    PartyUpdate,
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

    async fn list_parties(&self) -> Result<Vec<Party>, PartyError> {
        let rows = sqlx::query_as::<_, (String, String, i64, bool)>(
            "SELECT id::text, display_name, EXTRACT(EPOCH FROM created_at)::bigint, active
             FROM party.parties ORDER BY display_name, id",
        )
        .fetch_all(&self.pool)
        .await
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

        let contacts = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
            "SELECT id::text, party_id::text, name, email::text, phone FROM party.contacts
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
            })
            .collect())
    }
}
