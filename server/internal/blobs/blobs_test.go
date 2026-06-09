package blobs

import "testing"

func TestValidatePayload(t *testing.T) {
	if err := ValidatePayload([]byte{1}, make([]byte, NonceBytes), 10); err != nil {
		t.Fatalf("ValidatePayload valid error: %v", err)
	}
	if err := ValidatePayload(nil, make([]byte, NonceBytes), 10); err == nil {
		t.Fatal("empty ciphertext error = nil")
	}
	if err := ValidatePayload([]byte{1}, make([]byte, 11), 10); err == nil {
		t.Fatal("bad nonce error = nil")
	}
	if err := ValidatePayload([]byte{1, 2}, make([]byte, NonceBytes), 1); err == nil {
		t.Fatal("oversize error = nil")
	}
}

func TestValidateTaskID(t *testing.T) {
	if err := ValidateTaskID("vault_settings"); err != nil {
		t.Fatalf("ValidateTaskID vault_settings error: %v", err)
	}
	if err := ValidateTaskID(""); err == nil {
		t.Fatal("empty task id error = nil")
	}
}
