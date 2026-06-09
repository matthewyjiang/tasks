package serverhttp

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/matthewyjiang/tasks/server/internal/config"
)

func TestHealthz(t *testing.T) {
	router := NewRouter(Dependencies{Config: config.Config{JWTSecret: "secret"}})
	req := httptest.NewRequest(http.MethodGet, "/healthz", nil)
	rr := httptest.NewRecorder()

	router.ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusOK)
	}
	if !strings.Contains(rr.Body.String(), `"status":"ok"`) {
		t.Fatalf("body = %q, want health payload", rr.Body.String())
	}
}

func TestNotFoundReturnsJSON(t *testing.T) {
	router := NewRouter(Dependencies{Config: config.Config{JWTSecret: "secret"}})
	req := httptest.NewRequest(http.MethodGet, "/missing", nil)
	rr := httptest.NewRecorder()

	router.ServeHTTP(rr, req)

	if rr.Code != http.StatusNotFound {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusNotFound)
	}
	if !strings.Contains(rr.Body.String(), `"error":"not found"`) {
		t.Fatalf("body = %q, want JSON error", rr.Body.String())
	}
}
