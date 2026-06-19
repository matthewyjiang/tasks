package enrollment

import (
	"encoding/base64"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"

	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
	"github.com/matthewyjiang/tasks/server/internal/wire"
)

type Handler struct{ Repo Repository }

type createRequest struct {
	PubKey     wire.Base64Bytes `json:"pub_key"`
	DeviceName string           `json:"device_name"`
	Platform   string           `json:"platform"`
}

type createResponse struct {
	RequestID string `json:"request_id"`
	Status    string `json:"status"`
}

type requestResponse struct {
	RequestID  string           `json:"request_id"`
	PubKey     wire.Base64Bytes `json:"pub_key"`
	DeviceName string           `json:"device_name"`
	Platform   string           `json:"platform"`
	Status     string           `json:"status"`
	CreatedAt  string           `json:"created_at"`
}

type listResponse struct {
	Requests []requestResponse `json:"requests"`
}

type approveRequest struct {
	RecipientPubKey wire.Base64Bytes `json:"recipient_pub_key"`
	SenderPubKey    wire.Base64Bytes `json:"sender_pub_key"`
	WrappedKey      wire.Base64Bytes `json:"wrapped_key"`
	Nonce           wire.Base64Bytes `json:"nonce"`
}

type payloadResponse struct {
	RequestID       string           `json:"request_id"`
	SenderPubKey    wire.Base64Bytes `json:"sender_pub_key"`
	RecipientPubKey wire.Base64Bytes `json:"recipient_pub_key"`
	WrappedKey      wire.Base64Bytes `json:"wrapped_key"`
	Nonce           wire.Base64Bytes `json:"nonce"`
}

func (h Handler) Create(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	var req createRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if err := ValidatePubKey(req.PubKey.Bytes()); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	id, err := h.Repo.Create(r.Context(), userID, req.PubKey.Bytes(), req.DeviceName, req.Platform)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	respond.JSON(w, http.StatusCreated, createResponse{RequestID: id.String(), Status: "pending"})
}

func (h Handler) ListPending(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	reqs, err := h.Repo.ListPending(r.Context(), userID)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	resp := listResponse{Requests: make([]requestResponse, 0, len(reqs))}
	for _, req := range reqs {
		resp.Requests = append(resp.Requests, requestResponse{RequestID: req.ID.String(), PubKey: wire.Base64Bytes(req.PubKey), DeviceName: req.DeviceName, Platform: req.Platform, Status: req.Status, CreatedAt: req.CreatedAt.Format(http.TimeFormat)})
	}
	respond.JSON(w, http.StatusOK, resp)
}

func (h Handler) Approve(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	id, err := uuid.Parse(chi.URLParam(r, "request_id"))
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid request_id")
		return
	}
	var req approveRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if err := ValidatePubKey(req.RecipientPubKey.Bytes()); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := ValidatePubKey(req.SenderPubKey.Bytes()); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := ValidateWrappedKey(req.WrappedKey.Bytes()); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(req.Nonce.Bytes()) != 12 {
		respond.Error(w, http.StatusBadRequest, "12-byte nonce is required")
		return
	}
	if err := h.Repo.Approve(r.Context(), userID, id, req.RecipientPubKey.Bytes(), req.SenderPubKey.Bytes(), req.WrappedKey.Bytes(), req.Nonce.Bytes()); err != nil {
		if err == pgx.ErrNoRows {
			respond.Error(w, http.StatusNotFound, "request not found")
			return
		}
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h Handler) Reject(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	id, err := uuid.Parse(chi.URLParam(r, "request_id"))
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid request_id")
		return
	}
	if err := h.Repo.Reject(r.Context(), userID, id); err != nil {
		if err == pgx.ErrNoRows {
			respond.Error(w, http.StatusNotFound, "request not found")
			return
		}
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h Handler) Payload(w http.ResponseWriter, r *http.Request) {
	userID, _ := appmw.UserID(r.Context())
	pubKey, err := base64.StdEncoding.DecodeString(r.URL.Query().Get("pub_key"))
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid pub_key")
		return
	}
	if err := ValidatePubKey(pubKey); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	req, err := h.Repo.ApprovedForPubKey(r.Context(), userID, pubKey)
	if err != nil {
		if err == pgx.ErrNoRows {
			respond.Error(w, http.StatusNotFound, "payload not found")
			return
		}
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	respond.JSON(w, http.StatusOK, payloadResponse{RequestID: req.ID.String(), SenderPubKey: wire.Base64Bytes(req.SenderPubKey), RecipientPubKey: wire.Base64Bytes(req.PubKey), WrappedKey: wire.Base64Bytes(req.WrappedKey), Nonce: wire.Base64Bytes(req.Nonce)})
}
