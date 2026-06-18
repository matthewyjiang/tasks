-- +goose Up
CREATE TABLE enrollment_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pub_key BYTEA NOT NULL,
    device_name TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    wrapped_key BYTEA,
    nonce BYTEA,
    sender_pub_key BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT enrollment_status CHECK (status IN ('pending', 'approved', 'rejected')),
    CONSTRAINT enrollment_nonce_len CHECK (nonce IS NULL OR octet_length(nonce) = 12),
    CONSTRAINT enrollment_approved_payload_complete CHECK (
        status != 'approved' OR (
            wrapped_key IS NOT NULL AND nonce IS NOT NULL AND sender_pub_key IS NOT NULL
        )
    )
);
CREATE INDEX enrollment_requests_user_status_created ON enrollment_requests(user_id, status, created_at);

-- +goose Down
DROP TABLE IF EXISTS enrollment_requests;
