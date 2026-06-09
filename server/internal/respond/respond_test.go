package respond

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestJSONWritesStatusAndContentType(t *testing.T) {
	rr := httptest.NewRecorder()

	JSON(rr, http.StatusCreated, map[string]string{"ok": "yes"})

	if rr.Code != http.StatusCreated {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusCreated)
	}
	if got := rr.Header().Get("Content-Type"); got != "application/json" {
		t.Fatalf("content-type = %q, want application/json", got)
	}
	if !strings.Contains(rr.Body.String(), `"ok":"yes"`) {
		t.Fatalf("body = %q, want JSON payload", rr.Body.String())
	}
}

func TestErrorWritesErrorPayload(t *testing.T) {
	rr := httptest.NewRecorder()

	Error(rr, http.StatusBadRequest, "bad request")

	if rr.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusBadRequest)
	}
	if !strings.Contains(rr.Body.String(), `"error":"bad request"`) {
		t.Fatalf("body = %q, want error JSON", rr.Body.String())
	}
}

func TestDecodeJSONRejectsUnknownFields(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/", strings.NewReader(`{"name":"a","extra":true}`))
	var dst struct {
		Name string `json:"name"`
	}

	if err := DecodeJSON(req, &dst); err == nil {
		t.Fatal("DecodeJSON error = nil, want unknown-field error")
	}
}
