package service

import (
	"example.com/go-service/internal/repository"
)

type Service struct {
	repo *repository.Repository
}

func New() *Service {
	return &Service{repo: repository.New()}
}

func (s *Service) Authenticate(username, password string) bool {
	user := s.repo.FindByUsername(username)
	return user != nil
}
