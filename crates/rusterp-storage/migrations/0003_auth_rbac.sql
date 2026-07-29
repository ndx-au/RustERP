-- Classic RBAC: users, groups, roles, permissions.

CREATE TABLE auth.users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    login         CITEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    password_hash TEXT,
    last_login_at TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version   BIGINT NOT NULL DEFAULT 1,
    active        BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON auth.users
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE auth.groups (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL UNIQUE,
    description  TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_groups_updated_at
    BEFORE UPDATE ON auth.groups
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE auth.roles (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL UNIQUE,
    description  TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    row_version  BIGINT NOT NULL DEFAULT 1,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TRIGGER trg_roles_updated_at
    BEFORE UPDATE ON auth.roles
    FOR EACH ROW EXECUTE FUNCTION core.set_updated_at();

CREATE TABLE auth.permissions (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource  TEXT NOT NULL,
    action    TEXT NOT NULL,
    UNIQUE (resource, action)
);

CREATE TABLE auth.role_permissions (
    role_id       UUID NOT NULL REFERENCES auth.roles (id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES auth.permissions (id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE auth.user_groups (
    user_id  UUID NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES auth.groups (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, group_id)
);

CREATE TABLE auth.group_roles (
    group_id UUID NOT NULL REFERENCES auth.groups (id) ON DELETE CASCADE,
    role_id  UUID NOT NULL REFERENCES auth.roles (id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, role_id)
);

CREATE TABLE auth.user_roles (
    user_id UUID NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES auth.roles (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

INSERT INTO auth.permissions (resource, action) VALUES
    ('party', 'read'),
    ('party', 'write'),
    ('catalog', 'read'),
    ('catalog', 'write'),
    ('sales', 'read'),
    ('sales', 'write'),
    ('payment', 'read'),
    ('payment', 'write'),
    ('inventory', 'read'),
    ('inventory', 'write'),
    ('platform', 'admin');

INSERT INTO auth.roles (name, description) VALUES
    ('admin', 'Full platform access'),
    ('user', 'Standard user');

INSERT INTO auth.role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM auth.roles r
CROSS JOIN auth.permissions p
WHERE r.name = 'admin';
