-- +goose Up
DELETE FROM shared_blobs s
WHERE NOT EXISTS (
    SELECT 1
    FROM blobs b
    WHERE b.task_id = s.task_id AND b.owner_id = s.owner_id
);

ALTER TABLE shared_blobs DROP CONSTRAINT IF EXISTS shared_blobs_pkey;
ALTER TABLE shared_blobs ADD PRIMARY KEY (owner_id, task_id, recipient_id);

ALTER TABLE shared_blobs DROP CONSTRAINT IF EXISTS shared_blobs_blob_fk;
ALTER TABLE shared_blobs
    ADD CONSTRAINT shared_blobs_blob_fk
    FOREIGN KEY (task_id, owner_id) REFERENCES blobs(task_id, owner_id) ON DELETE CASCADE;

-- +goose Down
ALTER TABLE shared_blobs DROP CONSTRAINT IF EXISTS shared_blobs_blob_fk;
ALTER TABLE shared_blobs DROP CONSTRAINT IF EXISTS shared_blobs_pkey;
ALTER TABLE shared_blobs ADD PRIMARY KEY (task_id, recipient_id);
