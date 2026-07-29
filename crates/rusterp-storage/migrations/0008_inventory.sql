-- Inventory domain (toggleable module).

CREATE TYPE inventory.stock_move_state AS ENUM ('draft', 'confirmed', 'done', 'cancelled');

CREATE TABLE inventory.warehouses (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code         TEXT NOT NULL UNIQUE,
    name         TEXT NOT NULL,
    line1        TEXT,
    city         TEXT,
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

CREATE TRIGGER trg_warehouses_updated_at
    BEFORE UPDATE ON inventory.warehouses
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE inventory.stock_levels (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    warehouse_id  UUID NOT NULL REFERENCES inventory.warehouses (id) ON DELETE CASCADE,
    product_id    UUID NOT NULL REFERENCES catalog.products (id) ON DELETE CASCADE,
    qty_on_hand   NUMERIC(18, 6) NOT NULL DEFAULT 0,
    qty_reserved  NUMERIC(18, 6) NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version   BIGINT NOT NULL DEFAULT 1,
    UNIQUE (warehouse_id, product_id),
    CONSTRAINT chk_qty_non_negative CHECK (qty_on_hand >= 0 AND qty_reserved >= 0)
);

CREATE TRIGGER trg_stock_levels_updated_at
    BEFORE UPDATE ON inventory.stock_levels
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_stock_levels_nonzero ON inventory.stock_levels (warehouse_id, product_id)
    WHERE qty_on_hand <> 0 OR qty_reserved <> 0;

CREATE TABLE inventory.stock_moves (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id           UUID NOT NULL REFERENCES catalog.products (id),
    qty                  NUMERIC(18, 6) NOT NULL,
    from_warehouse_id    UUID REFERENCES inventory.warehouses (id),
    to_warehouse_id      UUID REFERENCES inventory.warehouses (id),
    state                inventory.stock_move_state NOT NULL DEFAULT 'draft',
    origin_document_kind sales.document_kind,
    origin_document_id   UUID,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by           UUID REFERENCES auth.users (id),
    updated_by           UUID REFERENCES auth.users (id),
    row_version          BIGINT NOT NULL DEFAULT 1,
    CONSTRAINT chk_stock_move_qty_positive CHECK (qty > 0)
);

CREATE TRIGGER trg_stock_moves_updated_at
    BEFORE UPDATE ON inventory.stock_moves
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE INDEX idx_stock_moves_product ON inventory.stock_moves (product_id, state);
CREATE INDEX idx_stock_moves_origin ON inventory.stock_moves (origin_document_kind, origin_document_id);
