.PHONY: run dev build clean install logs help

# Default target
help:
	@echo "Minik - GitHub Kanban App"
	@echo ""
	@echo "Available commands:"
	@echo "  make run       - Run the app in development mode"
	@echo "  make dev       - Same as 'make run'"
	@echo "  make build     - Build the app for production"
	@echo "  make install   - Build and install to /Applications"
	@echo "  make clean     - Clean build artifacts"
	@echo "  make logs      - Tail the application logs"
	@echo "  make help      - Show this help message"

# Run the app in development mode
run:
	cargo tauri dev

# Alias for run
dev: run

# Build for production
build:
	cargo tauri build

# Build and install to Applications folder
install:
	@echo "Building and installing minik.app to /Applications..."
	cargo tauri build --bundles app
	@cp -r src-tauri/target/release/bundle/macos/minik.app /Applications/
	@echo "✅ minik installed to /Applications"
	@echo "🚀 Opening minik..."
	@open /Applications/minik.app

# Clean build artifacts
clean:
	cd src-tauri && cargo clean
	rm -rf src-tauri/target

# View logs
logs:
	tail -f ~/Library/Logs/Minik/minik.log

# Run in release mode (faster)
release:
	cd src-tauri && cargo run --release

# Run linting
lint:
	cd src-tauri && cargo clippy -- -D warnings

# Run tests
test:
	cd src-tauri && cargo test

# Format code
fmt:
	cd src-tauri && cargo fmt

# Check formatting without applying
fmt-check:
	cd src-tauri && cargo fmt -- --check