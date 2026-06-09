package middleware

import (
	"net/http"
	"sync"
	"time"

	"github.com/google/uuid"

	"github.com/matthewyjiang/tasks/server/internal/respond"
)

type RateLimiter struct {
	limit   int
	window  time.Duration
	now     func() time.Time
	mu      sync.Mutex
	buckets map[uuid.UUID]bucket
}

type bucket struct {
	start time.Time
	count int
}

func NewRateLimiter(limit int, window time.Duration) *RateLimiter {
	return &RateLimiter{limit: limit, window: window, now: time.Now, buckets: make(map[uuid.UUID]bucket)}
}

func (l *RateLimiter) Allow(userID uuid.UUID) bool {
	if l.limit <= 0 {
		return true
	}
	l.mu.Lock()
	defer l.mu.Unlock()

	now := l.now()
	b := l.buckets[userID]
	if b.start.IsZero() || now.Sub(b.start) >= l.window {
		l.buckets[userID] = bucket{start: now, count: 1}
		return true
	}
	if b.count >= l.limit {
		return false
	}
	b.count++
	l.buckets[userID] = b
	return true
}

func (l *RateLimiter) Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		userID, ok := UserID(r.Context())
		if !ok {
			respond.Error(w, http.StatusUnauthorized, "missing authenticated user")
			return
		}
		if !l.Allow(userID) {
			respond.Error(w, http.StatusTooManyRequests, "rate limit exceeded")
			return
		}
		next.ServeHTTP(w, r)
	})
}
