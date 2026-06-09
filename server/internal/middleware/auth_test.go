package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

func TestRequireAuthAcceptsValidTokenAndStoresUserID(t *testing.T) {
	secret := []byte("secret")
	issuer := "issuer"
	wantUserID := uuid.New()
	tokenString := signedTestToken(t, secret, issuer, wantUserID, time.Hour)

	called := false
	handler := RequireAuth(secret, issuer)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		called = true
		gotUserID, ok := UserID(r.Context())
		if !ok {
			t.Fatal("UserID missing from context")
		}
		if gotUserID != wantUserID {
			t.Fatalf("userID = %s, want %s", gotUserID, wantUserID)
		}
		w.WriteHeader(http.StatusNoContent)
	}))

	req := httptest.NewRequest(http.MethodGet, "/protected", nil)
	req.Header.Set("Authorization", "Bearer "+tokenString)
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if !called {
		t.Fatal("next handler was not called")
	}
	if rr.Code != http.StatusNoContent {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusNoContent)
	}
}

func TestRequireAuthRejectsMissingToken(t *testing.T) {
	handler := RequireAuth([]byte("secret"), "issuer")(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		t.Fatal("next handler should not be called")
	}))

	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, httptest.NewRequest(http.MethodGet, "/protected", nil))

	if rr.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusUnauthorized)
	}
}

func TestRequireAuthRejectsWrongIssuer(t *testing.T) {
	tokenString := signedTestToken(t, []byte("secret"), "other", uuid.New(), time.Hour)
	handler := RequireAuth([]byte("secret"), "issuer")(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		t.Fatal("next handler should not be called")
	}))

	req := httptest.NewRequest(http.MethodGet, "/protected", nil)
	req.Header.Set("Authorization", "Bearer "+tokenString)
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if rr.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusUnauthorized)
	}
}

func signedTestToken(t *testing.T, secret []byte, issuer string, userID uuid.UUID, ttl time.Duration) string {
	t.Helper()
	now := time.Now()
	claims := jwt.RegisteredClaims{
		Subject:   userID.String(),
		Issuer:    issuer,
		IssuedAt:  jwt.NewNumericDate(now),
		ExpiresAt: jwt.NewNumericDate(now.Add(ttl)),
	}
	token, err := jwt.NewWithClaims(jwt.SigningMethodHS256, claims).SignedString(secret)
	if err != nil {
		t.Fatalf("SignedString error: %v", err)
	}
	return token
}
