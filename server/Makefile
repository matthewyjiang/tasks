.PHONY: tidy fmt test build check run migrate docker-up docker-down

tidy:
	go mod tidy

fmt:
	gofmt -w ./cmd ./internal

test:
	go test ./...

build:
	go build ./...

check: tidy fmt test build

run:
	go run ./cmd/server

migrate:
	go run ./cmd/server -migrate=true

docker-up:
	docker compose up -d postgres

docker-down:
	docker compose down
