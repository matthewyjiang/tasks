package blobs

import (
	"context"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
)

func CleanupTombstones(ctx context.Context, db *pgxpool.Pool, retention time.Duration) (int64, error) {
	cutoff := time.Now().Add(-retention).UnixMilli()
	ct, err := db.Exec(ctx, `DELETE FROM blobs WHERE deleted=true AND updated_at < $1`, cutoff)
	if err != nil {
		return 0, err
	}
	return ct.RowsAffected(), nil
}

func StartTombstoneCleanup(ctx context.Context, db *pgxpool.Pool, retention, interval time.Duration) {
	if interval <= 0 {
		interval = time.Hour
	}
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				_, _ = CleanupTombstones(ctx, db, retention)
			}
		}
	}()
}
