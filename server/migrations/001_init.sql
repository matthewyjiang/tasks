-- +goose Up
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email       TEXT UNIQUE NOT NULL,
    password_h  TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE devices (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pub_key       BYTEA NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX devices_user_created ON devices(user_id, created_at);

CREATE TABLE blobs (
    task_id     TEXT NOT NULL,
    owner_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ciphertext  BYTEA,
    nonce       BYTEA,
    updated_at  BIGINT NOT NULL,
    deleted     BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (task_id, owner_id),
    CONSTRAINT blobs_nonce_len CHECK (nonce IS NULL OR octet_length(nonce) = 12),
    CONSTRAINT blobs_deleted_payload CHECK (
        (deleted = true) OR (ciphertext IS NOT NULL AND nonce IS NOT NULL)
    )
);

CREATE INDEX blobs_owner_updated ON blobs(owner_id, updated_at);
CREATE INDEX blobs_deleted_updated ON blobs(deleted, updated_at);

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_h     TEXT NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked     BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX refresh_tokens_user_expires ON refresh_tokens(user_id, expires_at);
CREATE INDEX refresh_tokens_token_h ON refresh_tokens(token_h);

-- +goose Down
DROP TABLE IF EXISTS refresh_tokens;
DROP TABLE IF EXISTS blobs;
DROP TABLE IF EXISTS devices;
DROP TABLE IF EXISTS users;
