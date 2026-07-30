//! Sales domain: quotes, orders, invoices, credit notes (PostgreSQL).

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Quote,
    Order,
    Invoice,
    CreditNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentStatus {
    Draft,
    Confirmed,
    Posted,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct SalesDocument {
    pub id: String,
    pub kind: DocumentKind,
    pub status: DocumentStatus,
    pub number: String,
    pub party_id: String,
    pub currency: String,
    pub total_minor: i64,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct SalesDocumentLine {
    pub id: String,
    pub document_id: String,
    pub line_no: i32,
    pub description: String,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub product_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSalesDocument {
    pub kind: DocumentKind,
    pub party_id: String,
    pub description: String,
    pub unit_price_minor: i64,
    pub product_id: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SalesError {
    NotFound(String),
    Invalid(String),
}

impl std::fmt::Display for SalesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for SalesError {}

#[async_trait]
pub trait SalesRepository: Send + Sync {
    async fn create_document(&self, new: NewSalesDocument) -> Result<SalesDocument, SalesError>;
    async fn list_documents(
        &self,
        kind_filter: Option<DocumentKind>,
    ) -> Result<Vec<SalesDocument>, SalesError>;
    async fn get_document(
        &self,
        id: &str,
    ) -> Result<(SalesDocument, Vec<SalesDocumentLine>), SalesError>;
    async fn set_status(
        &self,
        id: &str,
        status: DocumentStatus,
    ) -> Result<SalesDocument, SalesError>;
}

pub struct PostgresSalesRepository {
    pool: PgPool,
}

impl PostgresSalesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn kind_domain(kind: DocumentKind) -> &'static str {
        match kind {
            DocumentKind::Quote => "sales.quote",
            DocumentKind::Order => "sales.order",
            DocumentKind::Invoice => "sales.invoice",
            DocumentKind::CreditNote => "sales.credit_note",
        }
    }

    fn kind_to_db(kind: DocumentKind) -> &'static str {
        match kind {
            DocumentKind::Quote => "quote",
            DocumentKind::Order => "order",
            DocumentKind::Invoice => "invoice",
            DocumentKind::CreditNote => "credit_note",
        }
    }

    fn kind_from_db(s: &str) -> DocumentKind {
        match s {
            "order" => DocumentKind::Order,
            "invoice" => DocumentKind::Invoice,
            "credit_note" => DocumentKind::CreditNote,
            _ => DocumentKind::Quote,
        }
    }

    fn status_to_db(s: DocumentStatus) -> &'static str {
        match s {
            DocumentStatus::Draft => "draft",
            DocumentStatus::Confirmed => "confirmed",
            DocumentStatus::Posted => "posted",
            DocumentStatus::Cancelled => "cancelled",
        }
    }

    fn status_from_db(s: &str) -> DocumentStatus {
        match s {
            "confirmed" => DocumentStatus::Confirmed,
            "posted" => DocumentStatus::Posted,
            "cancelled" => DocumentStatus::Cancelled,
            _ => DocumentStatus::Draft,
        }
    }

    async fn next_number(&self, kind: DocumentKind) -> Result<String, SalesError> {
        let domain = Self::kind_domain(kind);
        let row = sqlx::query_as::<_, (String, i64, i32)>(
            "UPDATE core.document_sequences
             SET next_value = next_value + 1
             WHERE domain = $1
             RETURNING prefix, next_value - 1, pad_width",
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SalesError::Invalid(e.to_string()))?
        .ok_or_else(|| SalesError::Invalid(format!("missing sequence for {domain}")))?;
        Ok(format!("{}{:0width$}", row.0, row.1, width = row.2 as usize))
    }
}

#[async_trait]
impl SalesRepository for PostgresSalesRepository {
    async fn create_document(&self, new: NewSalesDocument) -> Result<SalesDocument, SalesError> {
        let description = new.description.trim().to_string();
        if description.is_empty() {
            return Err(SalesError::Invalid("description required".into()));
        }
        let number = self.next_number(new.kind).await?;
        let id = Uuid::new_v4().to_string();
        let total = if new.kind == DocumentKind::CreditNote {
            -new.unit_price_minor.abs()
        } else {
            new.unit_price_minor
        };
        let mut txn = self
            .pool
            .begin()
            .await
            .map_err(|e| SalesError::Invalid(e.to_string()))?;
        sqlx::query(
            "INSERT INTO sales.sales_documents
             (id, kind, status, number, party_id, currency, subtotal_minor, tax_minor, total_minor, notes)
             VALUES ($1::uuid, $2::sales.document_kind, 'draft', $3, $4::uuid, 'AUD', $5, 0, $5, $6)",
        )
        .bind(&id)
        .bind(Self::kind_to_db(new.kind))
        .bind(&number)
        .bind(&new.party_id)
        .bind(total)
        .bind(&new.notes)
        .execute(&mut *txn)
        .await
        .map_err(|e| SalesError::Invalid(format!("insert document: {e}")))?;

        let line_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sales.sales_document_lines
             (id, document_id, line_no, product_id, description, quantity, unit_price_minor, subtotal_minor, tax_minor, total_minor)
             VALUES ($1::uuid, $2::uuid, 1, $3::uuid, $4, 1, $5, $5, 0, $5)",
        )
        .bind(&line_id)
        .bind(&id)
        .bind(&new.product_id)
        .bind(&description)
        .bind(total)
        .execute(&mut *txn)
        .await
        .map_err(|e| SalesError::Invalid(format!("insert line: {e}")))?;
        txn.commit()
            .await
            .map_err(|e| SalesError::Invalid(e.to_string()))?;
        Ok(SalesDocument {
            id,
            kind: new.kind,
            status: DocumentStatus::Draft,
            number,
            party_id: new.party_id,
            currency: "AUD".into(),
            total_minor: total,
            notes: new.notes,
        })
    }

    async fn list_documents(
        &self,
        kind_filter: Option<DocumentKind>,
    ) -> Result<Vec<SalesDocument>, SalesError> {
        let rows = if let Some(kind) = kind_filter {
            sqlx::query_as::<_, (String, String, String, String, String, String, i64, Option<String>)>(
                "SELECT id::text, kind::text, status::text, number, party_id::text, currency, total_minor, notes
                 FROM sales.sales_documents WHERE kind = $1::sales.document_kind
                 ORDER BY created_at DESC",
            )
            .bind(Self::kind_to_db(kind))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, (String, String, String, String, String, String, i64, Option<String>)>(
                "SELECT id::text, kind::text, status::text, number, party_id::text, currency, total_minor, notes
                 FROM sales.sales_documents ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| SalesError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| SalesDocument {
                id: r.0,
                kind: Self::kind_from_db(&r.1),
                status: Self::status_from_db(&r.2),
                number: r.3,
                party_id: r.4,
                currency: r.5,
                total_minor: r.6,
                notes: r.7.unwrap_or_default(),
            })
            .collect())
    }

    async fn get_document(
        &self,
        id: &str,
    ) -> Result<(SalesDocument, Vec<SalesDocumentLine>), SalesError> {
        let row = sqlx::query_as::<_, (String, String, String, String, String, String, i64, Option<String>)>(
            "SELECT id::text, kind::text, status::text, number, party_id::text, currency, total_minor, notes
             FROM sales.sales_documents WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SalesError::Invalid(e.to_string()))?
        .ok_or_else(|| SalesError::NotFound(format!("document {id}")))?;
        let doc = SalesDocument {
            id: row.0,
            kind: Self::kind_from_db(&row.1),
            status: Self::status_from_db(&row.2),
            number: row.3,
            party_id: row.4,
            currency: row.5,
            total_minor: row.6,
            notes: row.7.unwrap_or_default(),
        };
        let lines = sqlx::query_as::<_, (String, String, i32, String, i64, i64, Option<String>)>(
            "SELECT id::text, document_id::text, line_no, description, unit_price_minor, total_minor, product_id::text
             FROM sales.sales_document_lines WHERE document_id = $1::uuid ORDER BY line_no",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SalesError::Invalid(e.to_string()))?;
        Ok((
            doc,
            lines
                .into_iter()
                .map(|r| SalesDocumentLine {
                    id: r.0,
                    document_id: r.1,
                    line_no: r.2,
                    description: r.3,
                    unit_price_minor: r.4,
                    total_minor: r.5,
                    product_id: r.6,
                })
                .collect(),
        ))
    }

    async fn set_status(
        &self,
        id: &str,
        status: DocumentStatus,
    ) -> Result<SalesDocument, SalesError> {
        let (doc, _) = self.get_document(id).await?;
        let ok = matches!(
            (doc.status, status),
            (DocumentStatus::Draft, DocumentStatus::Confirmed)
                | (DocumentStatus::Confirmed, DocumentStatus::Posted)
                | (DocumentStatus::Draft, DocumentStatus::Cancelled)
                | (DocumentStatus::Confirmed, DocumentStatus::Cancelled)
        );
        if !ok {
            return Err(SalesError::Invalid(format!(
                "cannot transition {:?} -> {:?}",
                doc.status, status
            )));
        }
        sqlx::query(
            "UPDATE sales.sales_documents SET status = $1::sales.document_status, row_version = row_version + 1
             WHERE id = $2::uuid",
        )
        .bind(Self::status_to_db(status))
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| SalesError::Invalid(e.to_string()))?;
        let (doc, _) = self.get_document(id).await?;
        Ok(doc)
    }
}
