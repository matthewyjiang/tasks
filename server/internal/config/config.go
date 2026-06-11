package config

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

const (
	DefaultPort                 = "18080"
	DefaultJWTIssuer            = "tasks-server"
	DefaultAccessTokenTTL       = 15 * time.Minute
	DefaultRefreshTokenTTL      = 30 * 24 * time.Hour
	DefaultWriteRateLimitPerMin = 60
	DefaultMaxBlobBytes         = 1 << 20 // 1 MiB
	DefaultMaxBatchBlobs        = 100
	DefaultTombstoneRetention   = 30 * 24 * time.Hour
)

type Config struct {
	Port                 string
	DatabaseURL          string
	JWTSecret            string
	JWTIssuer            string
	AccessTokenTTL       time.Duration
	RefreshTokenTTL      time.Duration
	WriteRateLimitPerMin int
	MaxBlobBytes         int64
	MaxBatchBlobs        int
	TombstoneRetention   time.Duration
}

func Load() (Config, error) {
	cfg := Config{
		Port:                 getEnv("PORT", DefaultPort),
		DatabaseURL:          os.Getenv("DATABASE_URL"),
		JWTSecret:            os.Getenv("JWT_SECRET"),
		JWTIssuer:            getEnv("JWT_ISSUER", DefaultJWTIssuer),
		AccessTokenTTL:       getDurationEnv("ACCESS_TOKEN_TTL", DefaultAccessTokenTTL),
		RefreshTokenTTL:      getDurationEnv("REFRESH_TOKEN_TTL", DefaultRefreshTokenTTL),
		WriteRateLimitPerMin: getIntEnv("WRITE_RATE_LIMIT_PER_MIN", DefaultWriteRateLimitPerMin),
		MaxBlobBytes:         int64(getIntEnv("MAX_BLOB_BYTES", DefaultMaxBlobBytes)),
		MaxBatchBlobs:        getIntEnv("MAX_BATCH_BLOBS", DefaultMaxBatchBlobs),
		TombstoneRetention:   getDurationEnv("TOMBSTONE_RETENTION", DefaultTombstoneRetention),
	}

	if cfg.DatabaseURL == "" {
		return Config{}, fmt.Errorf("DATABASE_URL is required")
	}
	if cfg.JWTSecret == "" {
		return Config{}, fmt.Errorf("JWT_SECRET is required")
	}
	if cfg.WriteRateLimitPerMin <= 0 {
		return Config{}, fmt.Errorf("WRITE_RATE_LIMIT_PER_MIN must be positive")
	}
	if cfg.MaxBlobBytes <= 0 {
		return Config{}, fmt.Errorf("MAX_BLOB_BYTES must be positive")
	}
	if cfg.MaxBatchBlobs <= 0 {
		return Config{}, fmt.Errorf("MAX_BATCH_BLOBS must be positive")
	}

	return cfg, nil
}

func getEnv(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}

func getIntEnv(key string, fallback int) int {
	value := os.Getenv(key)
	if value == "" {
		return fallback
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return fallback
	}
	return parsed
}

func getDurationEnv(key string, fallback time.Duration) time.Duration {
	value := os.Getenv(key)
	if value == "" {
		return fallback
	}
	parsed, err := time.ParseDuration(value)
	if err != nil {
		return fallback
	}
	return parsed
}
