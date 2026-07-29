-- Core platform foundation: schemas, shared triggers, module registry, settings, audit.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS citext;

CREATE SCHEMA IF NOT EXISTS core;
CREATE SCHEMA IF NOT EXISTS auth;
CREATE SCHEMA IF NOT EXISTS party;
CREATE SCHEMA IF NOT EXISTS catalog;
CREATE SCHEMA IF NOT EXISTS sales;
CREATE SCHEMA IF NOT EXISTS payment;
CREATE SCHEMA IF NOT EXISTS inventory;
CREATE SCHEMA IF NOT EXISTS purchase;
CREATE SCHEMA IF NOT EXISTS accounting;
CREATE SCHEMA IF NOT EXISTS crm;
CREATE SCHEMA IF NOT EXISTS project;
CREATE SCHEMA IF NOT EXISTS hr;
CREATE SCHEMA IF NOT EXISTS manufacturing;

COMMENT ON SCHEMA core IS 'Platform: modules, settings, sequences, audit';
COMMENT ON SCHEMA auth IS 'Identity and RBAC';
COMMENT ON SCHEMA party IS 'Parties: customers, suppliers, prospects, contacts';
COMMENT ON SCHEMA catalog IS 'Products, categories, pricing';
COMMENT ON SCHEMA sales IS 'Quotes, orders, invoices, credit notes';
COMMENT ON SCHEMA payment IS 'Bank accounts, payments, allocations';
COMMENT ON SCHEMA inventory IS 'Warehouses, stock levels, moves (toggleable)';
COMMENT ON SCHEMA purchase IS 'Post-MVP: purchasing';
COMMENT ON SCHEMA accounting IS 'Post-MVP: chart of accounts (not full GL)';
COMMENT ON SCHEMA crm IS 'Post-MVP: CRM activities';
COMMENT ON SCHEMA project IS 'Post-MVP: project management';
COMMENT ON SCHEMA hr IS 'Post-MVP: human resources';
COMMENT ON SCHEMA manufacturing IS 'Post-MVP: manufacturing / BOMs';

CREATE OR REPLACE FUNCTION core.set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE core.modules (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    always_on   BOOLEAN NOT NULL DEFAULT FALSE,
    enabled     BOOLEAN NOT NULL DEFAULT FALSE,
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_modules_updated_at
    BEFORE UPDATE ON core.modules
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE core.settings (
    key         TEXT PRIMARY KEY,
    value       JSONB NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_settings_updated_at
    BEFORE UPDATE ON core.settings
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE core.document_sequences (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain      TEXT NOT NULL,
    prefix      TEXT NOT NULL DEFAULT '',
    next_value  BIGINT NOT NULL DEFAULT 1,
    pad_width   INT NOT NULL DEFAULT 6,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (domain, prefix)
);

CREATE TRIGGER trg_document_sequences_updated_at
    BEFORE UPDATE ON core.document_sequences
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE core.audit_events (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_id     UUID,
    schema_name  TEXT NOT NULL,
    table_name   TEXT NOT NULL,
    record_id    UUID NOT NULL,
    action       TEXT NOT NULL,
    changes      JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX idx_audit_events_occurred_at ON core.audit_events (occurred_at DESC);
CREATE INDEX idx_audit_events_record ON core.audit_events (schema_name, table_name, record_id);

INSERT INTO core.modules (id, name, always_on, enabled) VALUES
    ('core', 'Core Platform', TRUE, TRUE),
    ('parties', 'Parties', FALSE, TRUE),
    ('catalog', 'Catalog', FALSE, TRUE),
    ('sales', 'Sales', FALSE, TRUE),
    ('payments', 'Payments & Banking', FALSE, TRUE),
    ('inventory', 'Inventory', FALSE, FALSE);

INSERT INTO core.settings (key, value, description) VALUES
    ('org.default_currency', '"AUD"'::jsonb, 'Default ISO 4217 currency code');

-- Clean break from bootstrap migration 0001.
DROP TABLE IF EXISTS public.contacts CASCADE;
DROP TABLE IF EXISTS public.party_roles CASCADE;
DROP TABLE IF EXISTS public.parties CASCADE;
