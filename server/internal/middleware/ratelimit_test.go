package middleware

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestRateLimiterAllow(t *testing.T) {
	limiter := NewRateLimiter(2, time.Minute)
	userID := uuid.New()
	now := time.Unix(100, 0)
	limiter.now = func() time.Time { return now }

	if !limiter.Allow(userID) {
		t.Fatal("first request rejected")
	}
	if !limiter.Allow(userID) {
		t.Fatal("second request rejected")
	}
	if limiter.Allow(userID) {
		t.Fatal("third request allowed, want limited")
	}

	now = now.Add(time.Minute)
	if !limiter.Allow(userID) {
		t.Fatal("request after window rejected")
	}
}

func TestRateLimiterBucketsArePerUser(t *testing.T) {
	limiter := NewRateLimiter(1, time.Minute)
	limiter.now = func() time.Time { return time.Unix(100, 0) }

	if !limiter.Allow(uuid.New()) {
		t.Fatal("first user rejected")
	}
	if !limiter.Allow(uuid.New()) {
		t.Fatal("second user rejected")
	}
}
