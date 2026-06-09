package share

import (
	"testing"

	"github.com/google/uuid"
)

func TestValidateShare(t *testing.T) {
	if err := ValidateShare("task", uuid.New(), []byte{1}, make([]byte, NonceBytes)); err != nil {
		t.Fatalf("ValidateShare valid error: %v", err)
	}
	if err := ValidateShare("", uuid.New(), []byte{1}, make([]byte, NonceBytes)); err == nil {
		t.Fatal("empty task_id error = nil")
	}
	if err := ValidateShare("task", uuid.Nil, []byte{1}, make([]byte, NonceBytes)); err == nil {
		t.Fatal("nil recipient error = nil")
	}
	if err := ValidateShare("task", uuid.New(), nil, make([]byte, NonceBytes)); err == nil {
		t.Fatal("empty wrapped_dek error = nil")
	}
	if err := ValidateShare("task", uuid.New(), []byte{1}, make([]byte, 11)); err == nil {
		t.Fatal("bad nonce error = nil")
	}
}
