package config

import (
	"testing"
	"time"
)

func TestLoadDefaults(t *testing.T) {
	t.Setenv("DATABASE_URL", "postgres://example")
	t.Setenv("JWT_SECRET", "secret")
	t.Setenv("PORT", "")
	t.Setenv("JWT_ISSUER", "")
	t.Setenv("ACCESS_TOKEN_TTL", "")
	t.Setenv("REFRESH_TOKEN_TTL", "")
	t.Setenv("WRITE_RATE_LIMIT_PER_MIN", "")
	t.Setenv("MAX_BLOB_BYTES", "")
	t.Setenv("MAX_BATCH_BLOBS", "")
	t.Setenv("TOMBSTONE_RETENTION", "")

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}
	if cfg.Port != DefaultPort || cfg.JWTIssuer != DefaultJWTIssuer {
		t.Fatalf("defaults not applied: %+v", cfg)
	}
	if cfg.AccessTokenTTL != DefaultAccessTokenTTL || cfg.RefreshTokenTTL != DefaultRefreshTokenTTL {
		t.Fatalf("ttl defaults not applied: %+v", cfg)
	}
}

func TestLoadOverrides(t *testing.T) {
	t.Setenv("DATABASE_URL", "postgres://example")
	t.Setenv("JWT_SECRET", "secret")
	t.Setenv("PORT", "9090")
	t.Setenv("ACCESS_TOKEN_TTL", "30m")
	t.Setenv("WRITE_RATE_LIMIT_PER_MIN", "12")

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}
	if cfg.Port != "9090" {
		t.Fatalf("Port = %q, want 9090", cfg.Port)
	}
	if cfg.AccessTokenTTL != 30*time.Minute {
		t.Fatalf("AccessTokenTTL = %v, want 30m", cfg.AccessTokenTTL)
	}
	if cfg.WriteRateLimitPerMin != 12 {
		t.Fatalf("WriteRateLimitPerMin = %d, want 12", cfg.WriteRateLimitPerMin)
	}
}

func TestLoadRequiresDatabaseURLAndJWTSecret(t *testing.T) {
	t.Setenv("DATABASE_URL", "")
	t.Setenv("JWT_SECRET", "")

	if _, err := Load(); err == nil {
		t.Fatal("Load error = nil, want required env error")
	}
}
