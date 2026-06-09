package blobs

import (
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"

	appmw "github.com/matthewyjiang/tasks/server/internal/middleware"
	"github.com/matthewyjiang/tasks/server/internal/respond"
	"github.com/matthewyjiang/tasks/server/internal/wire"
)

type Handler struct {
	Repo          Repository
	MaxBlobBytes  int64
	MaxBatchBlobs int
}

type blobResponse struct {
	TaskID     string           `json:"task_id"`
	Ciphertext wire.Base64Bytes `json:"ciphertext,omitempty"`
	Nonce      wire.Base64Bytes `json:"nonce,omitempty"`
	UpdatedAt  int64            `json:"updated_at"`
	Deleted    bool             `json:"deleted"`
}

type listResponse struct {
	Blobs  []blobResponse `json:"blobs"`
	Cursor int64          `json:"cursor"`
}

type putRequest struct {
	Ciphertext wire.Base64Bytes `json:"ciphertext"`
	Nonce      wire.Base64Bytes `json:"nonce"`
}

type putResponse struct {
	TaskID    string `json:"task_id"`
	UpdatedAt int64  `json:"updated_at"`
}

type batchRequest struct {
	Blobs []struct {
		TaskID     string           `json:"task_id"`
		Ciphertext wire.Base64Bytes `json:"ciphertext"`
		Nonce      wire.Base64Bytes `json:"nonce"`
	} `json:"blobs"`
}

type batchResult struct {
	TaskID    string `json:"task_id"`
	Status    string `json:"status"`
	UpdatedAt int64  `json:"updated_at,omitempty"`
	Error     string `json:"error,omitempty"`
}

type batchResponse struct {
	Results []batchResult `json:"results"`
}

func (h Handler) List(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	since, _ := strconv.ParseInt(r.URL.Query().Get("since"), 10, 64)
	items, cursor, err := h.Repo.ListSince(r.Context(), ownerID, since)
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	resp := listResponse{Blobs: make([]blobResponse, 0, len(items)), Cursor: cursor}
	for _, b := range items {
		resp.Blobs = append(resp.Blobs, toResponse(b))
	}
	respond.JSON(w, http.StatusOK, resp)
}

func (h Handler) Put(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	taskID := chi.URLParam(r, "task_id")
	var req putRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if err := ValidateTaskID(taskID); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := ValidatePayload(req.Ciphertext.Bytes(), req.Nonce.Bytes(), h.MaxBlobBytes); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	b, err := h.Repo.Upsert(r.Context(), ownerID, taskID, req.Ciphertext.Bytes(), req.Nonce.Bytes(), NowUnixMillis())
	if err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	respond.JSON(w, http.StatusOK, putResponse{TaskID: b.TaskID, UpdatedAt: b.UpdatedAt})
}

func (h Handler) Delete(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	taskID := chi.URLParam(r, "task_id")
	if err := ValidateTaskID(taskID); err != nil {
		respond.Error(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := h.Repo.Tombstone(r.Context(), ownerID, taskID, NowUnixMillis()); err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h Handler) Batch(w http.ResponseWriter, r *http.Request) {
	ownerID, _ := appmw.UserID(r.Context())
	var req batchRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	if len(req.Blobs) > h.MaxBatchBlobs {
		respond.Error(w, http.StatusBadRequest, "batch too large")
		return
	}
	resp := batchResponse{Results: make([]batchResult, 0, len(req.Blobs))}
	for _, item := range req.Blobs {
		if err := ValidateTaskID(item.TaskID); err != nil {
			resp.Results = append(resp.Results, batchResult{TaskID: item.TaskID, Status: "error", Error: err.Error()})
			continue
		}
		if err := ValidatePayload(item.Ciphertext.Bytes(), item.Nonce.Bytes(), h.MaxBlobBytes); err != nil {
			resp.Results = append(resp.Results, batchResult{TaskID: item.TaskID, Status: "error", Error: err.Error()})
			continue
		}
		b, err := h.Repo.Upsert(r.Context(), ownerID, item.TaskID, item.Ciphertext.Bytes(), item.Nonce.Bytes(), NowUnixMillis())
		if err != nil {
			resp.Results = append(resp.Results, batchResult{TaskID: item.TaskID, Status: "error", Error: "internal error"})
			continue
		}
		resp.Results = append(resp.Results, batchResult{TaskID: b.TaskID, Status: "ok", UpdatedAt: b.UpdatedAt})
	}
	respond.JSON(w, http.StatusOK, resp)
}

func toResponse(b Blob) blobResponse {
	return blobResponse{TaskID: b.TaskID, Ciphertext: wire.Base64Bytes(b.Ciphertext), Nonce: wire.Base64Bytes(b.Nonce), UpdatedAt: b.UpdatedAt, Deleted: b.Deleted}
}
