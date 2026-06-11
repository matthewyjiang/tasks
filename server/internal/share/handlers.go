package share

import (
	"errors"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"

	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
	"github.com/matthewyjiang/tasks/server/internal/wire"
)

type Handler struct{ Repo Repository }

type createRequest struct {
	RecipientID string           `json:"recipient_id"`
	WrappedDEK  wire.Base64Bytes `json:"wrapped_dek"`
	Nonce       wire.Base64Bytes `json:"nonce"`
}

type sharedResponse struct {
	TaskID      string           `json:"task_id"`
	OwnerID     string           `json:"owner_id"`
	RecipientID string           `json:"recipient_id"`
	WrappedDEK  wire.Base64Bytes `json:"wrapped_dek"`
	Nonce       wire.Base64Bytes `json:"nonce"`
}

type inboxResponse struct {
	Shared []sharedResponse `json:"shared"`
}

func (h Handler) Create(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	taskID := chi.URLParam(r, "task_id")
	var req createRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	recipientID, err := uuid.Parse(req.RecipientID)
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid recipient_id")
		return
	}
	item := SharedBlob{TaskID: taskID, RecipientID: recipientID, WrappedDEK: req.WrappedDEK.Bytes(), Nonce: req.Nonce.Bytes()}
	if err := ValidateShare(taskID, recipientID, item.WrappedDEK, item.Nonce); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := h.Repo.Upsert(r.Context(), ownerID, item); err != nil {
		if errors.Is(err, ErrBlobNotFound) {
			respond.Error(w, http.StatusNotFound, "task not found")
			return
		}
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusCreated)
}

func (h Handler) Inbox(w http.ResponseWriter, r *http.Request) {
	recipientID, _ := appmw.UserID(r.Context())
	items, err := h.Repo.Inbox(r.Context(), recipientID)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	resp := inboxResponse{Shared: make([]sharedResponse, 0, len(items))}
	for _, item := range items {
		resp.Shared = append(resp.Shared, toResponse(item))
	}
	respond.JSON(w, http.StatusOK, resp)
}

func (h Handler) Delete(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	taskID := chi.URLParam(r, "task_id")
	recipientID, err := uuid.Parse(chi.URLParam(r, "recipient_id"))
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid recipient_id")
		return
	}
	if err := h.Repo.Delete(r.Context(), ownerID, taskID, recipientID); err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func toResponse(item SharedBlob) sharedResponse {
	return sharedResponse{TaskID: item.TaskID, OwnerID: item.OwnerID.String(), RecipientID: item.RecipientID.String(), WrappedDEK: wire.Base64Bytes(item.WrappedDEK), Nonce: wire.Base64Bytes(item.Nonce)}
}
