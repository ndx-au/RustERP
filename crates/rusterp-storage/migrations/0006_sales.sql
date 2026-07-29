-- Sales domain: quotes, orders, invoices, credit notes.

CREATE TYPE sales.document_kind AS ENUM ('quote', 'order', 'invoice', 'credit_note');
CREATE TYPE sales.document_status AS ENUM ('draft', 'confirmed', 'posted', 'cancelled');

CREATE TABLE sales.sales_documents (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind                sales.document_kind NOT NULL,
    status              sales.document_status NOT NULL DEFAULT 'draft',
    number              TEXT NOT NULL,
    party_id            UUID NOT NULL REFERENCES party.parties (id),
    source_document_id  UUID REFERENCES sales.sales_documents (id),
    order_date          DATE NOT NULL DEFAULT CURRENT_DATE,
    due_date            DATE,
    currency            CHAR(3) NOT NULL DEFAULT 'AUD',
    subtotal_minor      BIGINT NOT NULL DEFAULT 0,
    tax_minor           BIGINT NOT NULL DEFAULT 0,
    total_minor         BIGINT NOT NULL DEFAULT 0,
    notes               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by          UUID REFERENCES auth.users (id),
    updated_by          UUID REFERENCES auth.users (id),
    row_version         BIGINT NOT NULL DEFAULT 1,
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (kind, number),
    CONSTRAINT chk_credit_note_total CHECK (
        kind <> 'credit_note' OR total_minor <= 0
    )
);

CREATE TRIGGER trg_sales_documents_updated_at
    BEFORE UPDATE ON sales.sales_documents
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_sales_documents_party ON sales.sales_documents (party_id, kind, status);
CREATE INDEX idx_sales_documents_source ON sales.sales_documents (source_document_id);

CREATE TABLE sales.sales_document_lines (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id      UUID NOT NULL REFERENCES sales.sales_documents (id) ON DELETE CASCADE,
    line_no          INT NOT NULL,
    product_id       UUID REFERENCES catalog.products (id),
    description      TEXT NOT NULL,
    quantity         NUMERIC(18, 6) NOT NULL DEFAULT 1,
    uom_id           UUID REFERENCES catalog.units_of_measure (id),
    unit_price_minor BIGINT NOT NULL DEFAULT 0,
    tax_rate_bps     INT NOT NULL DEFAULT 0,
    subtotal_minor   BIGINT NOT NULL DEFAULT 0,
    tax_minor        BIGINT NOT NULL DEFAULT 0,
    total_minor      BIGINT NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version      BIGINT NOT NULL DEFAULT 1,
    UNIQUE (document_id, line_no)
);

CREATE TRIGGER trg_sales_document_lines_updated_at
    BEFORE UPDATE ON sales.sales_document_lines
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_sales_document_lines_document ON sales.sales_document_lines (document_id);

INSERT INTO core.document_sequences (domain, prefix) VALUES
    ('sales.quote', 'Q'),
    ('sales.order', 'SO'),
    ('sales.invoice', 'INV'),
    ('sales.credit_note', 'CN');
