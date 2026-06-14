package blobs

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

var ErrNotFound = errors.New("blob not found")

const NonceBytes = 12

type Blob struct {
	TaskID     string
	OwnerID    uuid.UUID
	Ciphertext []byte
	Nonce      []byte
	UpdatedAt  int64
	Deleted    bool
}

type Repository struct{ DB *pgxpool.Pool }

func ValidateTaskID(taskID string) error {
	if taskID == "" {
		return fmt.Errorf("task_id is required")
	}
	if len(taskID) > 200 {
		return fmt.Errorf("task_id is too long")
	}
	return nil
}

func ValidatePayload(ciphertext, nonce []byte, maxBytes int64) error {
	if len(ciphertext) == 0 {
		return fmt.Errorf("ciphertext is required")
	}
	if int64(len(ciphertext)) > maxBytes {
		return fmt.Errorf("ciphertext exceeds max size")
	}
	if len(nonce) != NonceBytes {
		return fmt.Errorf("nonce must be %d bytes", NonceBytes)
	}
	return nil
}

func NowUnixMillis() int64 { return time.Now().UnixMilli() }

func (r Repository) ListSince(ctx context.Context, ownerID uuid.UUID, since int64) ([]Blob, int64, error) {
	rows, err := r.DB.Query(ctx, `
SELECT task_id, owner_id, ciphertext, nonce, updated_at, deleted
FROM blobs
WHERE owner_id=$1 AND updated_at > $2
UNION ALL
SELECT b.task_id, b.owner_id, b.ciphertext, b.nonce, b.updated_at, b.deleted
FROM shared_blobs s
JOIN blobs b ON b.task_id=s.task_id AND b.owner_id=s.owner_id
WHERE s.recipient_id=$1 AND b.updated_at > $2
ORDER BY updated_at ASC`, ownerID, since)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()

	var out []Blob
	var cursor int64 = since
	for rows.Next() {
		var b Blob
		if err := rows.Scan(&b.TaskID, &b.OwnerID, &b.Ciphertext, &b.Nonce, &b.UpdatedAt, &b.Deleted); err != nil {
			return nil, 0, err
		}
		if b.UpdatedAt > cursor {
			cursor = b.UpdatedAt
		}
		out = append(out, b)
	}
	return out, cursor, rows.Err()
}

func (r Repository) Upsert(ctx context.Context, ownerID uuid.UUID, taskID string, ciphertext, nonce []byte, updatedAt int64) (Blob, error) {
	var b Blob
	err := r.DB.QueryRow(ctx, `INSERT INTO blobs (task_id, owner_id, ciphertext, nonce, updated_at, deleted)
VALUES ($1, $2, $3, $4, $5, false)
ON CONFLICT (task_id, owner_id) DO UPDATE SET ciphertext=EXCLUDED.ciphertext, nonce=EXCLUDED.nonce, updated_at=EXCLUDED.updated_at, deleted=false
RETURNING task_id, owner_id, ciphertext, nonce, updated_at, deleted`, taskID, ownerID, ciphertext, nonce, updatedAt).Scan(&b.TaskID, &b.OwnerID, &b.Ciphertext, &b.Nonce, &b.UpdatedAt, &b.Deleted)
	return b, err
}

func (r Repository) Tombstone(ctx context.Context, ownerID uuid.UUID, taskID string, updatedAt int64) error {
	ct, err := r.DB.Exec(ctx, `UPDATE blobs SET ciphertext=NULL, nonce=NULL, updated_at=$3, deleted=true WHERE owner_id=$1 AND task_id=$2`, ownerID, taskID, updatedAt)
	if err != nil {
		return err
	}
	if ct.RowsAffected() == 0 {
		_, err = r.DB.Exec(ctx, `INSERT INTO blobs (task_id, owner_id, updated_at, deleted) VALUES ($1, $2, $3, true)`, taskID, ownerID, updatedAt)
	}
	return err
}

func scanOne(row pgx.Row) (Blob, error) {
	var b Blob
	err := row.Scan(&b.TaskID, &b.OwnerID, &b.Ciphertext, &b.Nonce, &b.UpdatedAt, &b.Deleted)
	return b, err
}
