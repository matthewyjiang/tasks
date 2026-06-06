-- +goose Up
CREATE TABLE shared_blobs (
    task_id       TEXT NOT NULL,
    owner_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wrapped_dek   BYTEA NOT NULL,
    nonce         BYTEA NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, recipient_id),
    CONSTRAINT shared_blobs_nonce_len CHECK (octet_length(nonce) = 12)
);

CREATE INDEX shared_blobs_recipient_created ON shared_blobs(recipient_id, created_at);
CREATE INDEX shared_blobs_owner_task ON shared_blobs(owner_id, task_id);

-- +goose Down
DROP TABLE IF EXISTS shared_blobs;
