package settings

import (
	"encoding/json"
	"testing"
)

func TestValidateSettings(t *testing.T) {
	if err := Validate(json.RawMessage(`{"theme":"dark"}`)); err != nil {
		t.Fatalf("Validate valid error: %v", err)
	}
	if err := Validate(nil); err == nil {
		t.Fatal("nil settings error = nil")
	}
	if err := Validate(json.RawMessage(`[]`)); err == nil {
		t.Fatal("array settings error = nil")
	}
	if err := Validate(json.RawMessage(`{"last_sync_cursor":123}`)); err == nil {
		t.Fatal("last_sync_cursor error = nil")
	}
}

func TestNormalizeTrimsWhitespace(t *testing.T) {
	got := string(Normalize(json.RawMessage("  {\"a\":1}\n")))
	if got != `{"a":1}` {
		t.Fatalf("Normalize = %q", got)
	}
}
