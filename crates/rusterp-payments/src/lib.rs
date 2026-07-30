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
