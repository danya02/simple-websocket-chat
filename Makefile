all: build-frontend build-backend run-backend

build-frontend:
	trunk build frontend/index.html --filehash false --public-url "/"

build-backend:
	cargo build --bin backend

run-backend:
	RUST_LOG=debug cargo run --bin backend
