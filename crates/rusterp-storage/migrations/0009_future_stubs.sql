-- Post-MVP schema stubs: minimal tables for future domain work.

CREATE TABLE purchase.purchase_orders (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE purchase.purchase_orders IS 'Stub: full PO header/lines in a future spec';

CREATE TABLE purchase.purchase_order_lines (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE purchase.purchase_order_lines IS 'Stub: purchase order line items';

CREATE TABLE accounting.accounts (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE accounting.accounts IS 'Stub: chart of accounts only — not full double-entry GL';

CREATE TABLE crm.activities (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE crm.activities IS 'Stub: CRM activities and follow-ups';

CREATE TABLE project.projects (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE project.projects IS 'Stub: project management';

CREATE TABLE hr.employees (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE hr.employees IS 'Stub: HR employee records';

CREATE TABLE manufacturing.boms (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE manufacturing.boms IS 'Stub: bills of materials';
