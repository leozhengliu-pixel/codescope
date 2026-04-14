SHELL := /bin/sh

ifneq (,$(wildcard .env))
include .env
export
endif

CARGO ?= cargo
API_ADDR ?= 127.0.0.1:3000

.PHONY: help fmt check test api dev-up dev-down dev-logs

help:
	@printf '%s\n' \
	  'make fmt       - format Rust code' \
	  'make check     - cargo check workspace' \
	  'make test      - cargo test workspace' \
	  'make api       - run sourcebot-api' \
	  'make dev-up    - start local postgres via docker compose' \
	  'make dev-down  - stop local postgres' \
	  'make dev-logs  - show postgres logs'

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check

test:
	$(CARGO) test

api:
	SOURCEBOT_BIND_ADDR=$(API_ADDR) $(CARGO) run -p sourcebot-api

dev-up:
	docker compose up -d postgres

dev-down:
	docker compose down

dev-logs:
	docker compose logs -f postgres
