package keys

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"

	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
	"github.com/matthewyjiang/tasks/server/internal/wire"
)

type Handler struct{ Repo Repository }

type putKeyRequest struct {
	PubKey wire.Base64Bytes `json:"pub_key"`
}

type keyResponse struct {
	DeviceID string           `json:"device_id"`
	PubKey   wire.Base64Bytes `json:"pub_key"`
}

type keysResponse struct {
	UserID string        `json:"user_id"`
	Keys   []keyResponse `json:"keys"`
}

func (h Handler) GetUserKeys(w http.ResponseWriter, r *http.Request) {
	userID, err := uuid.Parse(chi.URLParam(r, "user_id"))
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid user_id")
		return
	}
	keys, err := h.Repo.ListUserKeys(r.Context(), userID)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	resp := keysResponse{UserID: userID.String(), Keys: make([]keyResponse, 0, len(keys))}
	for _, key := range keys {
		resp.Keys = append(resp.Keys, keyResponse{DeviceID: key.DeviceID.String(), PubKey: wire.Base64Bytes(key.PubKey)})
	}
	respond.JSON(w, http.StatusOK, resp)
}

func (h Handler) PutMe(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	var req putKeyRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if err := ValidatePubKey(req.PubKey.Bytes()); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if _, err := h.Repo.AddDevice(r.Context(), userID, req.PubKey.Bytes()); err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
