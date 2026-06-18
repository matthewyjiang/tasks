package enrollment

import (
	"context"
	"crypto/elliptic"
	"fmt"
	"math/big"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type Request struct {
	ID           uuid.UUID
	UserID       uuid.UUID
	PubKey       []byte
	DeviceName   string
	Platform     string
	Status       string
	WrappedKey   []byte
	Nonce        []byte
	SenderPubKey []byte
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

type Repository struct{ DB *pgxpool.Pool }

const MaxWrappedKeyBytes = 4096

func ValidatePubKey(pubKey []byte) error {
	if len(pubKey) == 0 {
		return fmt.Errorf("pub_key is required")
	}
	if len(pubKey) != 65 || pubKey[0] != 0x04 {
		return fmt.Errorf("pub_key must be an uncompressed P-256 SEC1 public key")
	}
	x := new(big.Int).SetBytes(pubKey[1:33])
	y := new(big.Int).SetBytes(pubKey[33:65])
	if !elliptic.P256().IsOnCurve(x, y) {
		return fmt.Errorf("pub_key must be a valid P-256 public key")
	}
	return nil
}

func ValidateWrappedKey(wrappedKey []byte) error {
	if len(wrappedKey) == 0 {
		return fmt.Errorf("wrapped_key is required")
	}
	if len(wrappedKey) > MaxWrappedKeyBytes {
		return fmt.Errorf("wrapped_key is too large")
	}
	return nil
}

func Create(ctx context.Context, db *pgxpool.Pool, userID uuid.UUID, pubKey []byte, deviceName, platform string) (uuid.UUID, error) {
	var id uuid.UUID
	err := db.QueryRow(ctx, `
		WITH existing AS (
			SELECT id FROM enrollment_requests WHERE user_id=$1 AND pub_key=$2 AND status='pending' LIMIT 1
		), inserted AS (
			INSERT INTO enrollment_requests (user_id, pub_key, device_name, platform)
			SELECT $1,$2,$3,$4 WHERE NOT EXISTS (SELECT 1 FROM existing)
			RETURNING id
		)
		SELECT id FROM inserted UNION ALL SELECT id FROM existing LIMIT 1`, userID, pubKey, trim(deviceName), trim(platform)).Scan(&id)
	return id, err
}

func (r Repository) Create(ctx context.Context, userID uuid.UUID, pubKey []byte, deviceName, platform string) (uuid.UUID, error) {
	return Create(ctx, r.DB, userID, pubKey, deviceName, platform)
}

func (r Repository) ListPending(ctx context.Context, userID uuid.UUID) ([]Request, error) {
	rows, err := r.DB.Query(ctx, `SELECT id,user_id,pub_key,device_name,platform,status,created_at,updated_at FROM enrollment_requests WHERE user_id=$1 AND status='pending' ORDER BY created_at ASC`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Request
	for rows.Next() {
		var req Request
		if err := rows.Scan(&req.ID, &req.UserID, &req.PubKey, &req.DeviceName, &req.Platform, &req.Status, &req.CreatedAt, &req.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, req)
	}
	return out, rows.Err()
}

func (r Repository) Approve(ctx context.Context, userID, id uuid.UUID, recipientPubKey, senderPubKey, wrappedKey, nonce []byte) error {
	ct, err := r.DB.Exec(ctx, `UPDATE enrollment_requests SET status='approved', sender_pub_key=$1, wrapped_key=$2, nonce=$3, updated_at=now() WHERE id=$4 AND user_id=$5 AND pub_key=$6 AND status='pending'`, senderPubKey, wrappedKey, nonce, id, userID, recipientPubKey)
	if err != nil {
		return err
	}
	if ct.RowsAffected() == 0 {
		return pgx.ErrNoRows
	}
	return nil
}

func (r Repository) Reject(ctx context.Context, userID, id uuid.UUID) error {
	ct, err := r.DB.Exec(ctx, `UPDATE enrollment_requests SET status='rejected', updated_at=now() WHERE id=$1 AND user_id=$2 AND status='pending'`, id, userID)
	if err != nil {
		return err
	}
	if ct.RowsAffected() == 0 {
		return pgx.ErrNoRows
	}
	return nil
}

func (r Repository) ApprovedForPubKey(ctx context.Context, userID uuid.UUID, pubKey []byte) (Request, error) {
	var req Request
	err := r.DB.QueryRow(ctx, `SELECT id,user_id,pub_key,device_name,platform,status,wrapped_key,nonce,sender_pub_key,created_at,updated_at FROM enrollment_requests WHERE user_id=$1 AND pub_key=$2 AND status='approved' ORDER BY updated_at DESC LIMIT 1`, userID, pubKey).Scan(&req.ID, &req.UserID, &req.PubKey, &req.DeviceName, &req.Platform, &req.Status, &req.WrappedKey, &req.Nonce, &req.SenderPubKey, &req.CreatedAt, &req.UpdatedAt)
	return req, err
}

func trim(s string) string {
	s = strings.TrimSpace(s)
	if len(s) > 120 {
		return s[:120]
	}
	return s
}
