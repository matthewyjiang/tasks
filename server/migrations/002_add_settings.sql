-- +goose Up
CREATE TABLE plaintext_settings (
    owner_id    UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings    JSONB NOT NULL,
    updated_at  BIGINT NOT NULL
);

-- +goose Down
DROP TABLE IF EXISTS plaintext_settings;
