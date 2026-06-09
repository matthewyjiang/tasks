package settings

import (
	"encoding/json"
	"net/http"

	"github.com/matthewyjiang/tasks/server/internal/blobs"
	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
)

type Handler struct{ Repo Repository }

type getResponse struct {
	Settings  json.RawMessage `json:"settings"`
	UpdatedAt int64           `json:"updated_at"`
}

type putRequest struct {
	Settings json.RawMessage `json:"settings"`
}

func (h Handler) Get(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	item, ok, err := h.Repo.Get(r.Context(), ownerID)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	if !ok {
		respond.JSON(w, http.StatusOK, getResponse{Settings: json.RawMessage(`{}`), UpdatedAt: 0})
		return
	}
	respond.JSON(w, http.StatusOK, getResponse{Settings: item.Settings, UpdatedAt: item.UpdatedAt})
}

func (h Handler) Put(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	var req putRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if err := Validate(req.Settings); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	item, err := h.Repo.Put(r.Context(), ownerID, req.Settings, blobs.NowUnixMillis())
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	respond.JSON(w, http.StatusOK, getResponse{Settings: item.Settings, UpdatedAt: item.UpdatedAt})
}
