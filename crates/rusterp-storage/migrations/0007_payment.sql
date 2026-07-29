-- Payments & banking domain.

CREATE TYPE payment.payment_direction AS ENUM ('inbound', 'outbound');
CREATE TYPE payment.payment_method AS ENUM ('cash', 'card', 'bank_transfer', 'other');
CREATE TYPE payment.payment_status AS ENUM ('draft', 'posted', 'reconciled', 'cancelled');

CREATE TABLE payment.bank_accounts (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                   TEXT NOT NULL,
    bank_name              TEXT,
    account_number_masked  TEXT,
    currency               CHAR(3) NOT NULL DEFAULT 'AUD',
    ledger_account_code    TEXT,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by             UUID REFERENCES auth.users (id),
    updated_by             UUID REFERENCES auth.users (id),
    row_version            BIGINT NOT NULL DEFAULT 1,
    active                 BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_bank_accounts_updated_at
    BEFORE UPDATE ON payment.bank_accounts
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE payment.payments (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    direction        payment.payment_direction NOT NULL,
    party_id         UUID NOT NULL REFERENCES party.parties (id),
    bank_account_id  UUID REFERENCES payment.bank_accounts (id),
    amount_minor     BIGINT NOT NULL,
    currency         CHAR(3) NOT NULL DEFAULT 'AUD',
    paid_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    method           payment.payment_method NOT NULL DEFAULT 'bank_transfer',
    reference        TEXT,
    status           payment.payment_status NOT NULL DEFAULT 'draft',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by       UUID REFERENCES auth.users (id),
    updated_by       UUID REFERENCES auth.users (id),
    row_version      BIGINT NOT NULL DEFAULT 1,
    active           BOOLEAN NOT NULL DEFAULT TRUE,
    CONSTRAINT chk_payment_amount_nonzero CHECK (amount_minor <> 0)
);

CREATE TRIGGER trg_payments_updated_at
    BEFORE UPDATE ON payment.payments
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_payments_party ON payment.payments (party_id, status);
CREATE INDEX idx_payments_paid_at ON payment.payments (paid_at DESC);

CREATE TABLE payment.payment_allocations (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id       UUID NOT NULL REFERENCES payment.payments (id) ON DELETE CASCADE,
    document_id      UUID NOT NULL REFERENCES sales.sales_documents (id),
    amount_minor     BIGINT NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version      BIGINT NOT NULL DEFAULT 1,
    CONSTRAINT chk_allocation_amount_positive CHECK (amount_minor > 0),
    UNIQUE (payment_id, document_id)
);

CREATE TRIGGER trg_payment_allocations_updated_at
    BEFORE UPDATE ON payment.payment_allocations
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_payment_allocations_document ON payment.payment_allocations (document_id);
