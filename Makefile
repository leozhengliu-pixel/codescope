SHELL := /bin/sh

ifneq (,$(wildcard .env))
include .env
export
endif

CARGO ?= cargo
API_ADDR ?= 127.0.0.1:3000
SQLX_CLI_VERSION ?= 0.8.6
SQLX_CLI_ROOT ?= .sqlx-cli

.PHONY: help fmt check test api worker dev-up dev-down dev-logs sqlx-migrate sqlx-test-reset sqlx-test metadata-dev-bootstrap runtime-backup runtime-restore metadata-backup metadata-restore

help:
	@printf '%s\n' \
	  'make fmt       - format Rust code' \
	  'make check     - cargo check workspace' \
	  'make test      - cargo test workspace' \
	  'make api       - run sourcebot-api' \
	  'make worker    - run sourcebot-worker' \
	  'make sqlx-migrate - run SQLx database migrations for the metadata schema against DATABASE_URL' \
	  'make sqlx-test-reset - drop, recreate, and re-migrate the deterministic test metadata database via TEST_DATABASE_URL' \
	  'make sqlx-test - reset the deterministic test metadata database and run focused metadata storage tests' \
	  'make metadata-dev-bootstrap - wait for local Postgres, ensure the dedicated test metadata database exists, run migrations, and run focused metadata compatibility tests' \
	  'make runtime-backup - create a timestamped backup of the current local runtime state' \
	  'make runtime-restore BACKUP_DIR=/path/to/backup - restore the local runtime state from a captured backup directory' \
	  'make metadata-backup - create a timestamped backup of the current local metadata database' \
	  'make metadata-restore BACKUP_DIR=/path/to/backup - restore the local metadata database from a captured backup directory' \
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

worker:
	$(CARGO) run -p sourcebot-worker

dev-up:
	docker compose up -d postgres

dev-down:
	docker compose down

dev-logs:
	docker compose logs -f postgres

sqlx-migrate:
	@: "$${DATABASE_URL:?DATABASE_URL must be set}"
	$(CARGO) install --locked sqlx-cli --version $(SQLX_CLI_VERSION) --no-default-features --features rustls,postgres --root $(SQLX_CLI_ROOT)
	DATABASE_URL="$$DATABASE_URL" $(SQLX_CLI_ROOT)/bin/sqlx migrate run --source crates/api/migrations

sqlx-test-reset:
	@: "$${TEST_DATABASE_URL:?TEST_DATABASE_URL must be set}"
	@case "$$TEST_DATABASE_URL" in \
	  postgres://*@127.0.0.1:***@localhost:5432/sourcebot_test) ;; \
	  *) printf '%s\n' 'TEST_DATABASE_URL must target the dedicated local sourcebot_test database on 127.0.0.1 or localhost' >&2; exit 1 ;; \
	esac
	$(CARGO) install --locked sqlx-cli --version $(SQLX_CLI_VERSION) --no-default-features --features rustls,postgres --root $(SQLX_CLI_ROOT)
	DATABASE_URL="$$TEST_DATABASE_URL" $(SQLX_CLI_ROOT)/bin/sqlx database reset --source crates/api/migrations -y

sqlx-test:
	@: "$${TEST_DATABASE_URL:?TEST_DATABASE_URL must be set}"
	$(MAKE) sqlx-test-reset
	DATABASE_URL="$$TEST_DATABASE_URL" $(CARGO) test -p sourcebot-api --bin sourcebot-api storage::tests -- --nocapture

metadata-dev-bootstrap:
	bash scripts/bootstrap_local_metadata_dev.sh

runtime-backup:
	bash scripts/backup_local_runtime_state.sh backups/runtime

runtime-restore:
	@: "$${BACKUP_DIR:?BACKUP_DIR must be set}"
	bash scripts/restore_local_runtime_state.sh "$$BACKUP_DIR"

metadata-backup:
	@: "$${DATABASE_URL:?DATABASE_URL must be set}"
	bash scripts/backup_local_metadata_db.sh backups/metadata

metadata-restore:
	@: "$${DATABASE_URL:?DATABASE_URL must be set}"
	@: "$${BACKUP_DIR:?BACKUP_DIR must be set}"
	bash scripts/restore_local_metadata_db.sh "$$BACKUP_DIR"
