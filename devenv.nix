{ pkgs, lib, config, inputs, ... }:

{
  # Project metadata
  name = "squirrel";

  # Environment variables
  env = {
    SQUIRREL_DEV = "1";
  };

  # Packages available in the development shell
  packages = with pkgs; [
    # Build tools
    git
    gnumake

    # SQLite with extensions
    sqlite

    # Documentation
    mdbook

    # Utilities
    jq
    ripgrep
    fd
  ];

  # Rust toolchain via fenix
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
  };

  # Python with packages
  languages.python = {
    enable = true;
    version = "3.12";

    venv = {
      enable = true;
      requirements = ''
        pydantic-ai
        httpx
        openai
        pytest
        pytest-asyncio
        ruff
      '';
    };
  };

  # Pre-commit hooks
  pre-commit.hooks = {
    # Rust
    rustfmt.enable = true;
    clippy.enable = true;

    # Python
    ruff.enable = true;

    # General
    check-merge-conflict.enable = true;
    end-of-file-fixer.enable = true;
    trim-trailing-whitespace.enable = true;
  };

  # Shell scripts available in the environment
  scripts = {
    # Run all tests
    test-all.exec = ''
      echo "Running Rust tests..."
      cargo test
      echo "Running Python tests..."
      pytest agent/tests/
    '';

    # Start daemon in development mode
    dev-daemon.exec = ''
      cargo run --bin sqrl-daemon -- --dev
    '';

    # Format all code
    fmt.exec = ''
      cargo fmt
      ruff format agent/
    '';

    # Lint all code
    lint.exec = ''
      cargo clippy -- -D warnings
      ruff check agent/
    '';
  };

  # Processes (long-running services for development)
  processes = {
    # daemon.exec = "cargo watch -x 'run --bin sqrl-daemon'";
  };

  # Services (databases, etc.)
  # services.sqlite.enable = true;

  # Enter shell message
  enterShell = ''
    echo "Squirrel development environment"
    echo ""
    echo "Available commands:"
    echo "  test-all    - Run all tests"
    echo "  dev-daemon  - Start daemon in dev mode"
    echo "  fmt         - Format all code"
    echo "  lint        - Lint all code"
    echo ""
    echo "See specs/ for project specifications"
  '';

  # Ensure minimum devenv version
  devenv.flakesIntegration = true;
}
