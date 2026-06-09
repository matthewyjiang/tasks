package auth

import (
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

func TestHashAndVerifyPassword(t *testing.T) {
	hash, err := HashPassword("correct horse battery staple")
	if err != nil {
		t.Fatalf("HashPassword error: %v", err)
	}

	ok, err := VerifyPassword("correct horse battery staple", hash)
	if err != nil {
		t.Fatalf("VerifyPassword error: %v", err)
	}
	if !ok {
		t.Fatal("VerifyPassword returned false for correct password")
	}

	ok, err = VerifyPassword("wrong", hash)
	if err != nil {
		t.Fatalf("VerifyPassword wrong password error: %v", err)
	}
	if ok {
		t.Fatal("VerifyPassword returned true for wrong password")
	}
}

func TestAccessTokenContainsRegisteredClaims(t *testing.T) {
	userID := uuid.New()
	svc := Service{
		JWTSecret:      []byte("test-secret"),
		JWTIssuer:      "test-issuer",
		AccessTokenTTL: time.Minute,
	}

	tokenString, err := svc.AccessToken(userID)
	if err != nil {
		t.Fatalf("AccessToken error: %v", err)
	}

	claims := jwt.RegisteredClaims{}
	token, err := jwt.ParseWithClaims(tokenString, &claims, func(token *jwt.Token) (any, error) {
		return []byte("test-secret"), nil
	})
	if err != nil {
		t.Fatalf("ParseWithClaims error: %v", err)
	}
	if !token.Valid {
		t.Fatal("token.Valid = false")
	}
	if claims.Subject != userID.String() {
		t.Fatalf("subject = %q, want %q", claims.Subject, userID.String())
	}
	if claims.Issuer != "test-issuer" {
		t.Fatalf("issuer = %q, want test-issuer", claims.Issuer)
	}
	if claims.ExpiresAt == nil || time.Until(claims.ExpiresAt.Time) <= 0 {
		t.Fatalf("expires_at = %v, want future time", claims.ExpiresAt)
	}
}
