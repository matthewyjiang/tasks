package serverhttp

import (
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/matthewyjiang/tasks/server/internal/auth"
	"github.com/matthewyjiang/tasks/server/internal/blobs"
	"github.com/matthewyjiang/tasks/server/internal/config"
	"github.com/matthewyjiang/tasks/server/internal/keys"
	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
	"github.com/matthewyjiang/tasks/server/internal/settings"
	"github.com/matthewyjiang/tasks/server/internal/share"
)

type Dependencies struct {
	Config config.Config
	DB     *pgxpool.Pool
}

func NewRouter(deps Dependencies) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	r.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		respond.JSON(w, http.StatusOK, map[string]string{"status": "ok"})
	})

	authHandler := auth.Handler{Service: auth.Service{
		DB:              deps.DB,
		JWTSecret:       []byte(deps.Config.JWTSecret),
		JWTIssuer:       deps.Config.JWTIssuer,
		AccessTokenTTL:  deps.Config.AccessTokenTTL,
		RefreshTokenTTL: deps.Config.RefreshTokenTTL,
	}}
	r.Post("/auth/register", authHandler.Register)
	r.Post("/auth/login", authHandler.Login)
	r.Post("/auth/refresh", authHandler.Refresh)
	r.Delete("/auth/session", authHandler.DeleteSession)

	blobHandler := blobs.Handler{Repo: blobs.Repository{DB: deps.DB}, MaxBlobBytes: deps.Config.MaxBlobBytes, MaxBatchBlobs: deps.Config.MaxBatchBlobs}
	keyHandler := keys.Handler{Repo: keys.Repository{DB: deps.DB}}
	shareHandler := share.Handler{Repo: share.Repository{DB: deps.DB}}
	settingsHandler := settings.Handler{Repo: settings.Repository{DB: deps.DB}}
	writeLimiter := appmw.NewRateLimiter(deps.Config.WriteRateLimitPerMin, time.Minute)
	r.Group(func(r chi.Router) {
		r.Use(appmw.RequireAuth([]byte(deps.Config.JWTSecret), deps.Config.JWTIssuer))
		r.Get("/blobs", blobHandler.List)
		r.With(writeLimiter.Middleware).Put("/blobs/{task_id}", blobHandler.Put)
		r.With(writeLimiter.Middleware).Delete("/blobs/{task_id}", blobHandler.Delete)
		r.With(writeLimiter.Middleware).Post("/blobs/batch", blobHandler.Batch)
		r.Get("/keys/{user_id}", keyHandler.GetUserKeys)
		r.With(writeLimiter.Middleware).Put("/keys/me", keyHandler.PutMe)
		r.With(writeLimiter.Middleware).Post("/share/{task_id}", shareHandler.Create)
		r.Get("/share/inbox", shareHandler.Inbox)
		r.With(writeLimiter.Middleware).Delete("/share/{task_id}/{recipient_id}", shareHandler.Delete)
		r.Get("/settings/plaintext", settingsHandler.Get)
		r.With(writeLimiter.Middleware).Put("/settings/plaintext", settingsHandler.Put)
	})

	r.NotFound(func(w http.ResponseWriter, r *http.Request) {
		respond.Error(w, http.StatusNotFound, "not found")
	})
	r.MethodNotAllowed(func(w http.ResponseWriter, r *http.Request) {
		respond.Error(w, http.StatusMethodNotAllowed, "method not allowed")
	})

	return r
}
