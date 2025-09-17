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

# Build and install to system
install:
	@echo "Building minik..."
	@if [ -n "$$OS" ] && [ "$$OS" = "Windows_NT" ]; then \
		echo "Installing for Windows..."; \
		cargo tauri build; \
		echo "Creating Programs directory if it doesn't exist..."; \
		mkdir -p "$$LOCALAPPDATA/Programs" 2>/dev/null || true; \
		echo "Installing minik.exe to %LOCALAPPDATA%\\Programs\\..."; \
		cp src-tauri/target/release/minik-app.exe "$$LOCALAPPDATA/Programs/minik.exe"; \
		echo "✅ minik installed to %LOCALAPPDATA%\\Programs\\minik.exe"; \
		echo "🚀 Starting minik..."; \
		"$$LOCALAPPDATA/Programs/minik.exe" & \
	else \
		UNAME="$$(uname)"; \
		if [ "$$UNAME" = "Darwin" ]; then \
			echo "Installing for macOS to /Applications..."; \
			cargo tauri build --bundles app; \
			cp -r src-tauri/target/release/bundle/macos/minik.app /Applications/; \
			echo "✅ minik installed to /Applications"; \
			echo "🚀 Opening minik..."; \
			open /Applications/minik.app; \
		elif [ "$$UNAME" = "Linux" ] || [ "$$UNAME" = "FreeBSD" ] || [ "$$UNAME" = "OpenBSD" ] || [ "$$UNAME" = "NetBSD" ] || [ "$$UNAME" = "DragonFly" ]; then \
			echo "Installing for $$UNAME..."; \
			cargo tauri build; \
			if command -v sudo >/dev/null 2>&1; then \
				SUDO_CMD="sudo"; \
			elif command -v doas >/dev/null 2>&1; then \
				SUDO_CMD="doas"; \
			else \
				echo "❌ Error: Neither sudo nor doas found. Please run as root or install sudo/doas."; \
				exit 1; \
			fi; \
			echo "Installing minik binary to /usr/local/bin..."; \
			$$SUDO_CMD install -m 755 src-tauri/target/release/minik-app /usr/local/bin/minik; \
			echo "✅ minik installed to /usr/local/bin"; \
			echo "🚀 You can now run 'minik' from the terminal"; \
		else \
			echo "❌ Unsupported platform: $$UNAME"; \
			exit 1; \
		fi; \
	fi

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

# Run strict linting focused on reliability and correctness
lint:
	cd src-tauri && cargo clippy -- \
		-D warnings \
		-D clippy::all \
		-D clippy::correctness \
		-D clippy::suspicious \
		-D clippy::complexity \
		-D clippy::perf \
		-D clippy::style \
		-D clippy::unwrap_used \
		-D clippy::expect_used \
		-D clippy::panic \
		-D clippy::unimplemented \
		-D clippy::todo \
		-A clippy::module_name_repetitions \
		-A clippy::must_use_candidate \
		-A clippy::too_many_lines \
		-A clippy::missing_errors_doc \
		-A clippy::missing_panics_doc
	cd src-tauri && cargo fmt -- --check

# Run tests
test:
	cd src-tauri && cargo test

# Format code
fmt:
	cd src-tauri && cargo fmt

# Check formatting without applying
fmt-check:
	cd src-tauri && cargo fmt -- --check