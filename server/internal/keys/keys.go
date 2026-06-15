package keys

import (
	"context"
	"fmt"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

type DeviceKey struct {
	DeviceID uuid.UUID
	UserID   uuid.UUID
	PubKey   []byte
}

type Repository struct{ DB *pgxpool.Pool }

func ValidatePubKey(pubKey []byte) error {
	if len(pubKey) == 0 {
		return fmt.Errorf("pub_key is required")
	}
	if len(pubKey) > 4096 {
		return fmt.Errorf("pub_key is too large")
	}
	return nil
}

func (r Repository) AddDevice(ctx context.Context, userID uuid.UUID, pubKey []byte) (uuid.UUID, error) {
	var deviceID uuid.UUID
	err := r.DB.QueryRow(ctx, `INSERT INTO devices (user_id, pub_key) VALUES ($1, $2) RETURNING id`, userID, pubKey).Scan(&deviceID)
	return deviceID, err
}

func (r Repository) UserIDByEmail(ctx context.Context, email string) (uuid.UUID, error) {
	var userID uuid.UUID
	err := r.DB.QueryRow(ctx, `SELECT id FROM users WHERE email=lower(trim($1))`, email).Scan(&userID)
	return userID, err
}

func (r Repository) ListUserKeys(ctx context.Context, userID uuid.UUID) ([]DeviceKey, error) {
	rows, err := r.DB.Query(ctx, `SELECT id, user_id, pub_key FROM devices WHERE user_id=$1 ORDER BY created_at ASC`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []DeviceKey
	for rows.Next() {
		var key DeviceKey
		if err := rows.Scan(&key.DeviceID, &key.UserID, &key.PubKey); err != nil {
			return nil, err
		}
		out = append(out, key)
	}
	return out, rows.Err()
}
