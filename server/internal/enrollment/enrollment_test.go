package enrollment

import (
	"crypto/elliptic"
	"os"
	"strings"
	"testing"
)

func TestValidatePubKey(t *testing.T) {
	x, y := elliptic.P256().ScalarBaseMult([]byte{1})
	valid := elliptic.Marshal(elliptic.P256(), x, y)
	if err := ValidatePubKey(valid); err != nil {
		t.Fatalf("ValidatePubKey valid error: %v", err)
	}
	if err := ValidatePubKey(nil); err == nil {
		t.Fatal("empty pub_key error = nil")
	}
	if err := ValidatePubKey([]byte{1, 2, 3}); err == nil {
		t.Fatal("short pub_key error = nil")
	}
	invalidPoint := append([]byte(nil), valid...)
	invalidPoint[64] ^= 0xff
	if err := ValidatePubKey(invalidPoint); err == nil {
		t.Fatal("invalid P-256 point error = nil")
	}
}

func TestValidateWrappedKey(t *testing.T) {
	if err := ValidateWrappedKey([]byte{1, 2, 3}); err != nil {
		t.Fatalf("ValidateWrappedKey small key error: %v", err)
	}
	if err := ValidateWrappedKey(nil); err == nil {
		t.Fatal("empty wrapped_key error = nil")
	}
	oversized := make([]byte, MaxWrappedKeyBytes+1)
	if err := ValidateWrappedKey(oversized); err == nil {
		t.Fatal("oversized wrapped_key error = nil")
	}
}

func TestTrimLimitsEnrollmentMetadata(t *testing.T) {
	if got := trim("  linux laptop  "); got != "linux laptop" {
		t.Fatalf("trim whitespace = %q", got)
	}
	long := make([]byte, 121)
	for i := range long {
		long[i] = 'a'
	}
	if got := trim(string(long)); len(got) != 120 {
		t.Fatalf("trim length = %d, want 120", len(got))
	}
}

func TestMigrationConstrainsApprovedPayloadCompleteness(t *testing.T) {
	data, err := os.ReadFile("../../migrations/005_device_enrollment.sql")
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	sql := string(data)
	for _, snippet := range []string{
		"status IN ('pending', 'approved', 'rejected')",
		"enrollment_approved_payload_complete",
		"status != 'approved'",
		"wrapped_key IS NOT NULL AND nonce IS NOT NULL AND sender_pub_key IS NOT NULL",
	} {
		if !strings.Contains(sql, snippet) {
			t.Fatalf("migration missing %q", snippet)
		}
	}
}

func TestRepositoryQueriesDeduplicateApproveAndFetchApprovedEnrollmentRequests(t *testing.T) {
	data, err := os.ReadFile("enrollment.go")
	if err != nil {
		t.Fatalf("read repository: %v", err)
	}
	source := string(data)
	for _, snippet := range []string{
		"WHERE NOT EXISTS (SELECT 1 FROM existing)",
		"AND pub_key=$6 AND status='pending'",
		"WHERE user_id=$1 AND pub_key=$2 AND status='approved'",
		"ORDER BY updated_at DESC LIMIT 1",
	} {
		if !strings.Contains(source, snippet) {
			t.Fatalf("repository missing %q", snippet)
		}
	}
}
