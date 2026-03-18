package main

import (
	"fmt"

	"example.com/go-service/internal/handler"
	"example.com/go-service/internal/service"
)

func main() {
	svc := service.New()
	h := handler.New(svc)
	fmt.Println("Starting server with handler:", h)
}
