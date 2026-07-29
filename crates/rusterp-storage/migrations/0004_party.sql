-- Parties domain: customers, suppliers, prospects, contacts, addresses.

CREATE TYPE party.party_role AS ENUM ('customer', 'supplier', 'prospect');
CREATE TYPE party.address_kind AS ENUM ('billing', 'shipping', 'other');
CREATE TYPE party.identifier_kind AS ENUM ('abn', 'acn', 'vat', 'other');

CREATE TABLE party.parties (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    UUID REFERENCES auth.users (id),
    updated_by    UUID REFERENCES auth.users (id),
    row_version   BIGINT NOT NULL DEFAULT 1,
    active        BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_parties_updated_at
    BEFORE UPDATE ON party.parties
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_parties_display_name ON party.parties (display_name);
CREATE INDEX idx_parties_active ON party.parties (active) WHERE active = TRUE;

CREATE TABLE party.party_roles (
    party_id UUID NOT NULL REFERENCES party.parties (id) ON DELETE CASCADE,
    role     party.party_role NOT NULL,
    PRIMARY KEY (party_id, role)
);

CREATE TABLE party.contacts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id    UUID NOT NULL REFERENCES party.parties (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    email       CITEXT,
    phone       TEXT,
    is_primary  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by  UUID REFERENCES auth.users (id),
    updated_by  UUID REFERENCES auth.users (id),
    row_version BIGINT NOT NULL DEFAULT 1,
    active      BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_contacts_updated_at
    BEFORE UPDATE ON party.contacts
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_contacts_party_id ON party.contacts (party_id);

CREATE TABLE party.addresses (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id     UUID NOT NULL REFERENCES party.parties (id) ON DELETE CASCADE,
    kind         party.address_kind NOT NULL DEFAULT 'other',
    line1        TEXT NOT NULL,
    line2        TEXT,
    city         TEXT NOT NULL,
    state_region TEXT,
    postal_code  TEXT,
    country      CHAR(2) NOT NULL DEFAULT 'AU',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by   UUID REFERENCES auth.users (id),
    updated_by   UUID REFERENCES auth.users (id),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_addresses_updated_at
    BEFORE UPDATE ON party.addresses
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_addresses_party_id ON party.addresses (party_id);

CREATE TABLE party.party_identifiers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id    UUID NOT NULL REFERENCES party.parties (id) ON DELETE CASCADE,
    kind        party.identifier_kind NOT NULL,
    value       TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by  UUID REFERENCES auth.users (id),
    updated_by  UUID REFERENCES auth.users (id),
    row_version BIGINT NOT NULL DEFAULT 1,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (party_id, kind, value)
);

CREATE TRIGGER trg_party_identifiers_updated_at
    BEFORE UPDATE ON party.party_identifiers
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_party_identifiers_party_id ON party.party_identifiers (party_id);
