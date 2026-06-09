package share

import (
	"context"
	"fmt"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

const NonceBytes = 12

type SharedBlob struct {
	TaskID      string
	OwnerID     uuid.UUID
	RecipientID uuid.UUID
	WrappedDEK  []byte
	Nonce       []byte
}

type Repository struct{ DB *pgxpool.Pool }

func ValidateShare(taskID string, recipientID uuid.UUID, wrappedDEK, nonce []byte) error {
	if taskID == "" {
		return fmt.Errorf("task_id is required")
	}
	if recipientID == uuid.Nil {
		return fmt.Errorf("recipient_id is required")
	}
	if len(wrappedDEK) == 0 {
		return fmt.Errorf("wrapped_dek is required")
	}
	if len(wrappedDEK) > 4096 {
		return fmt.Errorf("wrapped_dek is too large")
	}
	if len(nonce) != NonceBytes {
		return fmt.Errorf("nonce must be %d bytes", NonceBytes)
	}
	return nil
}

func (r Repository) Upsert(ctx context.Context, ownerID uuid.UUID, item SharedBlob) error {
	_, err := r.DB.Exec(ctx, `INSERT INTO shared_blobs (task_id, owner_id, recipient_id, wrapped_dek, nonce)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (task_id, recipient_id) DO UPDATE SET owner_id=EXCLUDED.owner_id, wrapped_dek=EXCLUDED.wrapped_dek, nonce=EXCLUDED.nonce, created_at=now()`, item.TaskID, ownerID, item.RecipientID, item.WrappedDEK, item.Nonce)
	return err
}

func (r Repository) Inbox(ctx context.Context, recipientID uuid.UUID) ([]SharedBlob, error) {
	rows, err := r.DB.Query(ctx, `SELECT task_id, owner_id, recipient_id, wrapped_dek, nonce FROM shared_blobs WHERE recipient_id=$1 ORDER BY created_at ASC`, recipientID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []SharedBlob
	for rows.Next() {
		var item SharedBlob
		if err := rows.Scan(&item.TaskID, &item.OwnerID, &item.RecipientID, &item.WrappedDEK, &item.Nonce); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r Repository) Delete(ctx context.Context, ownerID uuid.UUID, taskID string, recipientID uuid.UUID) error {
	_, err := r.DB.Exec(ctx, `DELETE FROM shared_blobs WHERE owner_id=$1 AND task_id=$2 AND recipient_id=$3`, ownerID, taskID, recipientID)
	return err
}
