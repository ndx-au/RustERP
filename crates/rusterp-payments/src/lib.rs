//! Payments & banking domain (PostgreSQL).

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone)]
pub struct BankAccount {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub id: String,
    pub direction: PaymentDirection,
    pub party_id: String,
    pub bank_account_id: Option<String>,
    pub amount_minor: i64,
    pub currency: String,
    pub reference: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct PaymentAllocation {
    pub id: String,
    pub payment_id: String,
    pub document_id: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentError {
    NotFound(String),
    Invalid(String),
}

impl std::fmt::Display for PaymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for PaymentError {}

#[async_trait]
pub trait PaymentsRepository: Send + Sync {
    async fn create_bank_account(
        &self,
        name: String,
        currency: String,
    ) -> Result<BankAccount, PaymentError>;
    async fn list_bank_accounts(&self) -> Result<Vec<BankAccount>, PaymentError>;
    async fn update_bank_account(
        &self,
        id: &str,
        name: Option<String>,
        currency: Option<String>,
        active: Option<bool>,
    ) -> Result<BankAccount, PaymentError>;
    async fn create_payment(
        &self,
        direction: PaymentDirection,
        party_id: String,
        bank_account_id: Option<String>,
        amount_minor: i64,
        currency: String,
        reference: String,
    ) -> Result<Payment, PaymentError>;
    async fn list_payments(&self) -> Result<Vec<Payment>, PaymentError>;
    async fn update_payment(
        &self,
        id: &str,
        reference: Option<String>,
        bank_account_id: Option<Option<String>>,
    ) -> Result<Payment, PaymentError>;
    async fn create_allocation(
        &self,
        payment_id: String,
        document_id: String,
        amount_minor: i64,
    ) -> Result<PaymentAllocation, PaymentError>;
    async fn list_allocations(
        &self,
        payment_id: &str,
    ) -> Result<Vec<PaymentAllocation>, PaymentError>;
}

pub struct PostgresPaymentsRepository {
    pool: PgPool,
}

impl PostgresPaymentsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PaymentsRepository for PostgresPaymentsRepository {
    async fn create_bank_account(
        &self,
        name: String,
        currency: String,
    ) -> Result<BankAccount, PaymentError> {
        let name = name.trim().to_string();
        let currency = if currency.trim().is_empty() {
            "AUD".into()
        } else {
            currency.trim().to_uppercase()
        };
        if name.is_empty() {
            return Err(PaymentError::Invalid("name required".into()));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO payment.bank_accounts (id, name, currency) VALUES ($1::uuid, $2, $3)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&currency)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(BankAccount {
            id,
            name,
            currency,
            active: true,
        })
    }

    async fn list_bank_accounts(&self) -> Result<Vec<BankAccount>, PaymentError> {
        let rows = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT id::text, name, currency, active FROM payment.bank_accounts ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| BankAccount {
                id: r.0,
                name: r.1,
                currency: r.2,
                active: r.3,
            })
            .collect())
    }

    async fn update_bank_account(
        &self,
        id: &str,
        name: Option<String>,
        currency: Option<String>,
        active: Option<bool>,
    ) -> Result<BankAccount, PaymentError> {
        let current = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT id::text, name, currency, active FROM payment.bank_accounts WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?
        .ok_or_else(|| PaymentError::NotFound(format!("bank account {id}")))?;

        let name = match name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(PaymentError::Invalid("name required".into()));
                }
                n
            }
            None => current.1,
        };
        let currency = match currency {
            Some(c) => {
                let c = c.trim().to_uppercase();
                if c.len() != 3 {
                    return Err(PaymentError::Invalid("currency must be 3 letters".into()));
                }
                c
            }
            None => current.2,
        };
        let active = active.unwrap_or(current.3);

        sqlx::query(
            "UPDATE payment.bank_accounts SET name = $1, currency = $2, active = $3,
             row_version = row_version + 1 WHERE id = $4::uuid",
        )
        .bind(&name)
        .bind(&currency)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;

        Ok(BankAccount {
            id: current.0,
            name,
            currency,
            active,
        })
    }

    async fn create_payment(
        &self,
        direction: PaymentDirection,
        party_id: String,
        bank_account_id: Option<String>,
        amount_minor: i64,
        currency: String,
        reference: String,
    ) -> Result<Payment, PaymentError> {
        if amount_minor == 0 {
            return Err(PaymentError::Invalid("amount must be non-zero".into()));
        }
        let currency = if currency.trim().is_empty() {
            "AUD".into()
        } else {
            currency.trim().to_uppercase()
        };
        let dir = match direction {
            PaymentDirection::Inbound => "inbound",
            PaymentDirection::Outbound => "outbound",
        };
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO payment.payments
             (id, direction, party_id, bank_account_id, amount_minor, currency, reference, status)
             VALUES ($1::uuid, $2::payment.payment_direction, $3::uuid, $4::uuid, $5, $6, $7, 'posted')",
        )
        .bind(&id)
        .bind(dir)
        .bind(&party_id)
        .bind(&bank_account_id)
        .bind(amount_minor)
        .bind(&currency)
        .bind(&reference)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(Payment {
            id,
            direction,
            party_id,
            bank_account_id,
            amount_minor,
            currency,
            reference,
            status: "posted".into(),
        })
    }

    async fn list_payments(&self) -> Result<Vec<Payment>, PaymentError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64, String, Option<String>, String)>(
            "SELECT id::text, direction::text, party_id::text, bank_account_id::text, amount_minor, currency, reference, status::text
             FROM payment.payments ORDER BY paid_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| Payment {
                id: r.0,
                direction: if r.1 == "outbound" {
                    PaymentDirection::Outbound
                } else {
                    PaymentDirection::Inbound
                },
                party_id: r.2,
                bank_account_id: r.3,
                amount_minor: r.4,
                currency: r.5,
                reference: r.6.unwrap_or_default(),
                status: r.7,
            })
            .collect())
    }

    async fn update_payment(
        &self,
        id: &str,
        reference: Option<String>,
        bank_account_id: Option<Option<String>>,
    ) -> Result<Payment, PaymentError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64, String, Option<String>, String)>(
            "SELECT id::text, direction::text, party_id::text, bank_account_id::text, amount_minor, currency, reference, status::text
             FROM payment.payments WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?
        .ok_or_else(|| PaymentError::NotFound(format!("payment {id}")))?;

        let reference = reference.unwrap_or_else(|| rows.6.clone().unwrap_or_default());
        let bank_account_id = bank_account_id.unwrap_or_else(|| rows.3.clone());

        sqlx::query(
            "UPDATE payment.payments SET reference = $1, bank_account_id = $2::uuid,
             row_version = row_version + 1 WHERE id = $3::uuid",
        )
        .bind(&reference)
        .bind(&bank_account_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;

        Ok(Payment {
            id: rows.0,
            direction: if rows.1 == "outbound" {
                PaymentDirection::Outbound
            } else {
                PaymentDirection::Inbound
            },
            party_id: rows.2,
            bank_account_id,
            amount_minor: rows.4,
            currency: rows.5,
            reference,
            status: rows.7,
        })
    }

    async fn create_allocation(
        &self,
        payment_id: String,
        document_id: String,
        amount_minor: i64,
    ) -> Result<PaymentAllocation, PaymentError> {
        if amount_minor == 0 {
            return Err(PaymentError::Invalid("amount must be non-zero".into()));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO payment.payment_allocations (id, payment_id, document_id, amount_minor)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
        )
        .bind(&id)
        .bind(&payment_id)
        .bind(&document_id)
        .bind(amount_minor)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(PaymentAllocation {
            id,
            payment_id,
            document_id,
            amount_minor,
        })
    }

    async fn list_allocations(
        &self,
        payment_id: &str,
    ) -> Result<Vec<PaymentAllocation>, PaymentError> {
        let rows = sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT id::text, payment_id::text, document_id::text, amount_minor
             FROM payment.payment_allocations WHERE payment_id = $1::uuid ORDER BY created_at",
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| PaymentAllocation {
                id: r.0,
                payment_id: r.1,
                document_id: r.2,
                amount_minor: r.3,
            })
            .collect())
    }
}
