package blobs

import (
	"context"
	"testing"
	"time"
)

func TestStartTombstoneCleanupReturnsOnCancelledContext(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	StartTombstoneCleanup(ctx, nil, time.Hour, time.Millisecond)
	// This test documents that startup with a cancelled context is non-blocking.
}
