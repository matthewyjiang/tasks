package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
	"golang.org/x/crypto/argon2"
)

var ErrInvalidCredentials = errors.New("invalid credentials")

type Service struct {
	DB              *pgxpool.Pool
	JWTSecret       []byte
	JWTIssuer       string
	AccessTokenTTL  time.Duration
	RefreshTokenTTL time.Duration
}

type TokenPair struct {
	AccessToken  string
	RefreshToken string
	UserID       uuid.UUID
}

func (s Service) Register(ctx context.Context, email, password string, pubKey []byte) (TokenPair, error) {
	email = normalizeEmail(email)
	if email == "" || password == "" || len(pubKey) == 0 {
		return TokenPair{}, fmt.Errorf("email, password, and pub_key are required")
	}
	passwordHash, err := HashPassword(password)
	if err != nil {
		return TokenPair{}, err
	}

	tx, err := s.DB.Begin(ctx)
	if err != nil {
		return TokenPair{}, err
	}
	defer tx.Rollback(ctx)

	var userID uuid.UUID
	if err := tx.QueryRow(ctx, `INSERT INTO users (email, password_h) VALUES ($1, $2) RETURNING id`, email, passwordHash).Scan(&userID); err != nil {
		return TokenPair{}, err
	}
	if _, err := tx.Exec(ctx, `INSERT INTO devices (user_id, pub_key) VALUES ($1, $2)`, userID, pubKey); err != nil {
		return TokenPair{}, err
	}

	pair, err := s.issueTokensTx(ctx, tx, userID)
	if err != nil {
		return TokenPair{}, err
	}
	if err := tx.Commit(ctx); err != nil {
		return TokenPair{}, err
	}
	return pair, nil
}

func (s Service) Login(ctx context.Context, email, password string) (TokenPair, error) {
	email = normalizeEmail(email)
	var userID uuid.UUID
	var passwordHash string
	if err := s.DB.QueryRow(ctx, `SELECT id, password_h FROM users WHERE email=$1`, email).Scan(&userID, &passwordHash); err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return TokenPair{}, ErrInvalidCredentials
		}
		return TokenPair{}, err
	}
	ok, err := VerifyPassword(password, passwordHash)
	if err != nil || !ok {
		return TokenPair{}, ErrInvalidCredentials
	}
	return s.issueTokens(ctx, userID)
}

func (s Service) Refresh(ctx context.Context, refreshToken string) (TokenPair, error) {
	tokenHash := hashToken(refreshToken)
	tx, err := s.DB.Begin(ctx)
	if err != nil {
		return TokenPair{}, err
	}
	defer tx.Rollback(ctx)

	var userID uuid.UUID
	if err := tx.QueryRow(ctx, `SELECT user_id FROM refresh_tokens WHERE token_h=$1 AND revoked=false AND expires_at > now() FOR UPDATE`, tokenHash).Scan(&userID); err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return TokenPair{}, ErrInvalidCredentials
		}
		return TokenPair{}, err
	}
	if _, err := tx.Exec(ctx, `UPDATE refresh_tokens SET revoked=true WHERE token_h=$1`, tokenHash); err != nil {
		return TokenPair{}, err
	}
	pair, err := s.issueTokensTx(ctx, tx, userID)
	if err != nil {
		return TokenPair{}, err
	}
	if err := tx.Commit(ctx); err != nil {
		return TokenPair{}, err
	}
	return pair, nil
}

func (s Service) RevokeRefreshToken(ctx context.Context, refreshToken string) error {
	_, err := s.DB.Exec(ctx, `UPDATE refresh_tokens SET revoked=true WHERE token_h=$1`, hashToken(refreshToken))
	return err
}

func (s Service) issueTokens(ctx context.Context, userID uuid.UUID) (TokenPair, error) {
	tx, err := s.DB.Begin(ctx)
	if err != nil {
		return TokenPair{}, err
	}
	defer tx.Rollback(ctx)
	pair, err := s.issueTokensTx(ctx, tx, userID)
	if err != nil {
		return TokenPair{}, err
	}
	return pair, tx.Commit(ctx)
}

func (s Service) issueTokensTx(ctx context.Context, tx pgx.Tx, userID uuid.UUID) (TokenPair, error) {
	access, err := s.AccessToken(userID)
	if err != nil {
		return TokenPair{}, err
	}
	refresh, err := randomToken(32)
	if err != nil {
		return TokenPair{}, err
	}
	_, err = tx.Exec(ctx, `INSERT INTO refresh_tokens (user_id, token_h, expires_at) VALUES ($1, $2, $3)`, userID, hashToken(refresh), time.Now().Add(s.RefreshTokenTTL))
	if err != nil {
		return TokenPair{}, err
	}
	return TokenPair{AccessToken: access, RefreshToken: refresh, UserID: userID}, nil
}

func (s Service) AccessToken(userID uuid.UUID) (string, error) {
	now := time.Now()
	claims := jwt.RegisteredClaims{Subject: userID.String(), Issuer: s.JWTIssuer, IssuedAt: jwt.NewNumericDate(now), ExpiresAt: jwt.NewNumericDate(now.Add(s.AccessTokenTTL))}
	return jwt.NewWithClaims(jwt.SigningMethodHS256, claims).SignedString(s.JWTSecret)
}

func normalizeEmail(email string) string { return strings.ToLower(strings.TrimSpace(email)) }

func randomToken(n int) (string, error) {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

func hashToken(token string) string {
	sum := sha256.Sum256([]byte(token))
	return hex.EncodeToString(sum[:])
}

func HashPassword(password string) (string, error) {
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return "", err
	}
	hash := argon2.IDKey([]byte(password), salt, 1, 64*1024, 4, 32)
	return "argon2id$v=19$m=65536,t=1,p=4$" + base64.RawStdEncoding.EncodeToString(salt) + "$" + base64.RawStdEncoding.EncodeToString(hash), nil
}

func VerifyPassword(password, encoded string) (bool, error) {
	parts := strings.Split(encoded, "$")
	if len(parts) != 5 {
		return false, fmt.Errorf("invalid password hash")
	}
	salt, err := base64.RawStdEncoding.DecodeString(parts[3])
	if err != nil {
		return false, err
	}
	expected, err := base64.RawStdEncoding.DecodeString(parts[4])
	if err != nil {
		return false, err
	}
	actual := argon2.IDKey([]byte(password), salt, 1, 64*1024, 4, uint32(len(expected)))
	return subtleEqual(actual, expected), nil
}

func subtleEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	var v byte
	for i := range a {
		v |= a[i] ^ b[i]
	}
	return v == 0
}
