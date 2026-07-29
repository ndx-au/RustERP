CREATE TABLE IF NOT EXISTS parties (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at   BIGINT NOT NULL,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS party_roles (
    party_id TEXT NOT NULL REFERENCES parties(id),
    role     TEXT NOT NULL,
    PRIMARY KEY (party_id, role)
);

CREATE TABLE IF NOT EXISTS contacts (
    id       TEXT PRIMARY KEY,
    party_id TEXT NOT NULL REFERENCES parties(id),
    name     TEXT NOT NULL,
    email    TEXT,
    phone    TEXT
);
