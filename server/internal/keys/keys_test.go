package keys

import "testing"

func TestValidatePubKey(t *testing.T) {
	if err := ValidatePubKey([]byte{1, 2, 3}); err != nil {
		t.Fatalf("ValidatePubKey valid error: %v", err)
	}
	if err := ValidatePubKey(nil); err == nil {
		t.Fatal("empty pub_key error = nil")
	}
	if err := ValidatePubKey(make([]byte, 4097)); err == nil {
		t.Fatal("oversize pub_key error = nil")
	}
}
