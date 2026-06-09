.PHONY: tidy fmt fmt-check test build check fix run dev migrate docker-up docker-down

tidy:
	go mod tidy

fmt:
	gofmt -w ./cmd ./internal

fmt-check:
	@test -z "$$(gofmt -l ./cmd ./internal)" || (echo "Run 'make fmt' to format:"; gofmt -l ./cmd ./internal; exit 1)

test:
	go test ./...

build:
	go build ./...

check: fmt-check test build

fix: tidy fmt

run:
	go run ./cmd/server

# Start Postgres, load .env if present, and run the API with migrations.
dev: docker-up
	@if [ -f .env ]; then set -a; . ./.env; set +a; fi; \
	go run ./cmd/server

migrate:
	go run ./cmd/server -migrate=true

docker-up:
	docker compose up -d postgres

docker-down:
	docker compose down
