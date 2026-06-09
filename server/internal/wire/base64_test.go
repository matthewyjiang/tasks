package wire

import (
	"encoding/json"
	"testing"
)

func TestBase64BytesMarshalJSON(t *testing.T) {
	got, err := json.Marshal(struct {
		Ciphertext Base64Bytes `json:"ciphertext"`
	}{Ciphertext: Base64Bytes([]byte{1, 2, 3})})
	if err != nil {
		t.Fatalf("Marshal error: %v", err)
	}
	want := `{"ciphertext":"AQID"}`
	if string(got) != want {
		t.Fatalf("json = %s, want %s", got, want)
	}
}

func TestBase64BytesUnmarshalJSON(t *testing.T) {
	var dst struct {
		Nonce Base64Bytes `json:"nonce"`
	}
	if err := json.Unmarshal([]byte(`{"nonce":"AAECAwQFBgcICQoL"}`), &dst); err != nil {
		t.Fatalf("Unmarshal error: %v", err)
	}
	if len(dst.Nonce) != 12 {
		t.Fatalf("nonce len = %d, want 12", len(dst.Nonce))
	}
	for i, got := range dst.Nonce {
		if got != byte(i) {
			t.Fatalf("nonce[%d] = %d, want %d", i, got, i)
		}
	}
}

func TestBase64BytesRejectsInvalidBase64(t *testing.T) {
	var dst Base64Bytes
	if err := json.Unmarshal([]byte(`"not base64!"`), &dst); err == nil {
		t.Fatal("Unmarshal error = nil, want invalid base64 error")
	}
}
