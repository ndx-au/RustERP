-- Catalog domain: products, categories, units of measure, pricing.

CREATE TYPE catalog.product_type AS ENUM ('stock', 'service', 'consumable');

CREATE TABLE catalog.units_of_measure (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code           TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    ratio_to_base  NUMERIC(18, 6) NOT NULL DEFAULT 1,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version    BIGINT NOT NULL DEFAULT 1,
    active         BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_uom_updated_at
    BEFORE UPDATE ON catalog.units_of_measure
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE catalog.product_categories (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id    UUID REFERENCES catalog.product_categories (id),
    name         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_product_categories_updated_at
    BEFORE UPDATE ON catalog.product_categories
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_product_categories_parent ON catalog.product_categories (parent_id);

CREATE TABLE catalog.products (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type         catalog.product_type NOT NULL DEFAULT 'stock',
    sku          TEXT NOT NULL UNIQUE,
    name         TEXT NOT NULL,
    description  TEXT,
    category_id  UUID REFERENCES catalog.product_categories (id),
    uom_id       UUID NOT NULL REFERENCES catalog.units_of_measure (id),
    sale_ok      BOOLEAN NOT NULL DEFAULT TRUE,
    purchase_ok  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by   UUID REFERENCES auth.users (id),
    updated_by   UUID REFERENCES auth.users (id),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_products_updated_at
    BEFORE UPDATE ON catalog.products
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_products_name ON catalog.products (name);
CREATE INDEX idx_products_category ON catalog.products (category_id);

CREATE TABLE catalog.price_lists (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL UNIQUE,
    currency     CHAR(3) NOT NULL DEFAULT 'AUD',
    valid_from   TIMESTAMPTZ,
    valid_to     TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_price_lists_updated_at
    BEFORE UPDATE ON catalog.price_lists
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE catalog.prices (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    price_list_id  UUID NOT NULL REFERENCES catalog.price_lists (id) ON DELETE CASCADE,
    product_id     UUID NOT NULL REFERENCES catalog.products (id) ON DELETE CASCADE,
    amount_minor   BIGINT NOT NULL,
    currency       CHAR(3) NOT NULL DEFAULT 'AUD',
    valid_from     TIMESTAMPTZ,
    valid_to       TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version    BIGINT NOT NULL DEFAULT 1,
    active         BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (price_list_id, product_id, valid_from)
);

CREATE TRIGGER trg_prices_updated_at
    BEFORE UPDATE ON catalog.prices
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_prices_product ON catalog.prices (product_id);

-- Seed base unit of measure.
INSERT INTO catalog.units_of_measure (code, name) VALUES ('ea', 'Each');
