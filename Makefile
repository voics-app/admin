.PHONY: dev test lint build docker

IMAGE ?= discordy-admin:dev

dev:
	cd bff && cargo run & cd web && pnpm install && pnpm dev

test:
	cd bff && cargo test
	cd web && pnpm install --frozen-lockfile && pnpm test

lint:
	cd bff && cargo fmt --check && cargo clippy --all-targets -- -D warnings
	cd web && pnpm lint && pnpm typecheck

build:
	cd web && pnpm install --frozen-lockfile && pnpm build
	cd bff && cargo build --release

docker:
	docker build -t $(IMAGE) .

# Install the pre-push hook (lint + unit tests, see lefthook.yml).
.PHONY: hooks
hooks:
	lefthook install
