package wire

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
)

// Base64Bytes marshals binary API fields as standard base64 JSON strings.
type Base64Bytes []byte

func (b Base64Bytes) MarshalJSON() ([]byte, error) {
	return json.Marshal(base64.StdEncoding.EncodeToString([]byte(b)))
}

func (b *Base64Bytes) UnmarshalJSON(data []byte) error {
	var encoded string
	if err := json.Unmarshal(data, &encoded); err != nil {
		return err
	}
	decoded, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil {
		return fmt.Errorf("invalid base64: %w", err)
	}
	*b = decoded
	return nil
}

func (b Base64Bytes) Bytes() []byte {
	return []byte(b)
}
