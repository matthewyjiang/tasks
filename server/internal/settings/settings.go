package settings

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

const ForbiddenCursorKey = "last_sync_cursor"

type PlaintextSettings struct {
	OwnerID   uuid.UUID
	Settings  json.RawMessage
	UpdatedAt int64
}

type Repository struct{ DB *pgxpool.Pool }

func Validate(raw json.RawMessage) error {
	if len(raw) == 0 {
		return fmt.Errorf("settings is required")
	}
	if !json.Valid(raw) {
		return fmt.Errorf("settings must be valid json")
	}
	var obj map[string]json.RawMessage
	if err := json.Unmarshal(raw, &obj); err != nil {
		return fmt.Errorf("settings must be a json object")
	}
	if _, ok := obj[ForbiddenCursorKey]; ok {
		return fmt.Errorf("last_sync_cursor must not be persisted")
	}
	return nil
}

func Normalize(raw json.RawMessage) json.RawMessage {
	return bytes.TrimSpace(raw)
}

func (r Repository) Get(ctx context.Context, ownerID uuid.UUID) (PlaintextSettings, bool, error) {
	var out PlaintextSettings
	err := r.DB.QueryRow(ctx, `SELECT owner_id, settings, updated_at FROM plaintext_settings WHERE owner_id=$1`, ownerID).Scan(&out.OwnerID, &out.Settings, &out.UpdatedAt)
	if err != nil {
		if err.Error() == "no rows in result set" {
			return PlaintextSettings{}, false, nil
		}
		return PlaintextSettings{}, false, err
	}
	return out, true, nil
}

func (r Repository) Put(ctx context.Context, ownerID uuid.UUID, raw json.RawMessage, updatedAt int64) (PlaintextSettings, error) {
	var out PlaintextSettings
	err := r.DB.QueryRow(ctx, `INSERT INTO plaintext_settings (owner_id, settings, updated_at) VALUES ($1, $2, $3)
ON CONFLICT (owner_id) DO UPDATE SET settings=EXCLUDED.settings, updated_at=EXCLUDED.updated_at
RETURNING owner_id, settings, updated_at`, ownerID, Normalize(raw), updatedAt).Scan(&out.OwnerID, &out.Settings, &out.UpdatedAt)
	return out, err
}
