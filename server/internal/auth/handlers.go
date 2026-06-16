package auth

import (
	"encoding/base64"
	"errors"
	"net/http"

	"github.com/matthewyjiang/tasks/server/internal/respond"
)

type Handler struct{ Service Service }

type registerRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
	PubKey   string `json:"pub_key"`
}

type loginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type refreshRequest struct {
	RefreshToken string `json:"refresh_token"`
}

type tokenResponse struct {
	JWT          string `json:"jwt"`
	RefreshToken string `json:"refresh_token"`
	UserID       string `json:"user_id,omitempty"`
}

func (h Handler) Register(w http.ResponseWriter, r *http.Request) {
	var req registerRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	pubKey, err := base64.StdEncoding.DecodeString(req.PubKey)
	if err != nil {
		respond.Error(w, http.StatusBadRequest, "pub_key must be base64")
		return
	}
	pair, err := h.Service.Register(r.Context(), req.Email, req.Password, pubKey)
	if err != nil {
		status := http.StatusInternalServerError
		msg := "internal error"
		switch {
		case errors.Is(err, ErrEmailAlreadyRegistered):
			status, msg = http.StatusConflict, "email already registered"
		case errors.Is(err, ErrInvalidRegistration):
			status, msg = http.StatusBadRequest, err.Error()
		}
		respond.Error(w, status, msg)
		return
	}
	respond.JSON(w, http.StatusCreated, tokenResponse{JWT: pair.AccessToken, RefreshToken: pair.RefreshToken, UserID: pair.UserID.String()})
}

func (h Handler) Login(w http.ResponseWriter, r *http.Request) {
	var req loginRequest
	if err := respond.DecodeJSON(r, &req); err != nil {
		respond.Error(w, http.StatusBadRequest, "invalid json")
		return
	}
	pair, err := h.Service.Login(r.Context(), req.Email, req.Password)
	if err != nil {
		status := http.StatusInternalServerError
		msg := "internal error"
		if errors.Is(err, ErrInvalidCredentials) {
			status, msg = http.StatusUnauthorized, "invalid credentials"
		}
		respond.Error(w, status, msg)
		return
	}
	respond.JSON(w, http.StatusOK, tokenResponse{JWT: pair.AccessToken, RefreshToken: pair.RefreshToken})
}

func (h Handler) Refresh(w http.ResponseWriter, r *http.Request) {
	var req refreshRequest
	if err := respond.DecodeJSON(r, &req); err != nil || req.RefreshToken == "" {
		respond.Error(w, http.StatusBadRequest, "refresh_token is required")
		return
	}
	pair, err := h.Service.Refresh(r.Context(), req.RefreshToken)
	if err != nil {
		status := http.StatusInternalServerError
		msg := "internal error"
		if errors.Is(err, ErrInvalidCredentials) {
			status, msg = http.StatusUnauthorized, "invalid refresh token"
		}
		respond.Error(w, status, msg)
		return
	}
	respond.JSON(w, http.StatusOK, tokenResponse{JWT: pair.AccessToken, RefreshToken: pair.RefreshToken})
}

func (h Handler) DeleteSession(w http.ResponseWriter, r *http.Request) {
	var req refreshRequest
	if err := respond.DecodeJSON(r, &req); err != nil || req.RefreshToken == "" {
		respond.Error(w, http.StatusBadRequest, "refresh_token is required")
		return
	}
	if err := h.Service.RevokeRefreshToken(r.Context(), req.RefreshToken); err != nil {
		respond.Error(w, http.StatusInternalServerError, "internal error")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
