package main

import (
	"context"
	"errors"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"github.com/matthewyjiang/tasks/server/internal/blobs"
	"github.com/matthewyjiang/tasks/server/internal/config"
	"github.com/matthewyjiang/tasks/server/internal/db"
	serverhttp "github.com/matthewyjiang/tasks/server/internal/http"
)

func main() {
	migrate := flag.Bool("migrate", true, "run database migrations before starting")
	migrationsDir := flag.String("migrations", defaultMigrationsDir(), "path to goose SQL migrations")
	flag.Parse()

	cfg, err := config.Load()
	if err != nil {
		log.Fatalf("load config: %v", err)
	}

	if *migrate {
		if err := db.Migrate(cfg.DatabaseURL, *migrationsDir); err != nil {
			log.Fatalf("migrate database: %v", err)
		}
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	pool, err := db.Open(ctx, cfg.DatabaseURL)
	if err != nil {
		log.Fatalf("open database: %v", err)
	}
	defer pool.Close()
	blobs.StartTombstoneCleanup(ctx, pool, cfg.TombstoneRetention, time.Hour)

	srv := &http.Server{
		Addr:              ":" + cfg.Port,
		Handler:           serverhttp.NewRouter(serverhttp.Dependencies{Config: cfg, DB: pool}),
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		log.Printf("server listening on %s", srv.Addr)
		if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("serve: %v", err)
		}
	}()

	<-ctx.Done()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := srv.Shutdown(shutdownCtx); err != nil {
		log.Printf("shutdown: %v", err)
	}
}

func defaultMigrationsDir() string {
	if _, err := os.Stat("migrations"); err == nil {
		return "migrations"
	}
	return filepath.Join("server", "migrations")
}
