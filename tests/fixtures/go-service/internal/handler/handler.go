package handler

import (
	"example.com/go-service/internal/service"
)

type Handler struct {
	svc *service.Service
}

func New(svc *service.Service) *Handler {
	return &Handler{svc: svc}
}

func (h *Handler) HandleLogin(username, password string) bool {
	return h.svc.Authenticate(username, password)
}
