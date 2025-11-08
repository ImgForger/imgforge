# Project Structure

## Source Code Organization

```
src/
├── main.rs              # Binary entry point
├── lib.rs               # Library root with module declarations
├── server.rs            # Server initialization and routing
├── config.rs            # Environment-based configuration
├── constants.rs         # Application constants
├── handlers.rs          # HTTP request handlers
├── middleware.rs        # Custom middleware (rate limiting, metrics, etc.)
├── monitoring.rs        # Prometheus metrics and health checks
├── fetch.rs             # Image fetching utilities
├── url.rs               # URL parsing and validation
├── caching/             # Caching subsystem
│   ├── mod.rs
│   ├── cache.rs         # Cache implementation
│   ├── config.rs        # Cache configuration
│   └── error.rs         # Cache-specific errors
└── processing/          # Image processing pipeline
    ├── mod.rs
    ├── options.rs       # Processing options parsing
    ├── presets.rs       # Named preset handling
    ├── transform.rs     # Core image transformations
    ├── save.rs          # Image output handling
    ├── utils.rs         # Processing utilities
    └── tests.rs         # Processing unit tests
```

## Configuration and Deployment

```
deployment/              # Deployment scripts and utilities
├── deploy.sh            # One-line deployment script
├── deploy-systemd.sh    # SystemD service deployment
├── upgrade-systemd.sh   # SystemD service upgrades
└── uninstall*.sh        # Cleanup scripts

.cargo/config.toml       # Cargo build configuration
.rustfmt.toml            # Code formatting rules (max_width = 120)
Dockerfile               # Multi-stage container build
docker-compose.yml       # Local development stack
Makefile                 # Build automation
```

## Documentation and Testing

```
doc/                         # Comprehensive documentation
├── 1_installation.md        # Setup instructions
├── 2_quick_start.md         # Getting started guide
├── 3_configuration.md       # Configuration reference
├── 4_url_structure.md       # URL format specification
├── 5_processing_options.md  # Image processing options
├── 6_request_lifecycle.md   # Request flow documentation
└── ...

tests/                  # Integration tests
├── handlers_integration_tests.rs
├── presets_integration_tests.rs
└── handlers_integration_tests_extended.rs

loadtest/              # Performance testing utilities
```

## Monitoring and Operations

```
grafana-dashboards/     # Pre-built Grafana dashboards
prometheus.yml          # Prometheus configuration
grafana-datasources.yml # Grafana data source config
MONITORING.md           # Monitoring setup guide
```

## Module Architecture Patterns

- **Modular design**: Each major feature area has its own module
- **Error handling**: Custom error types per module with thiserror
- **Configuration**: Centralized environment-based config in `config.rs`
- **State management**: Shared application state via Arc<AppState>
- **Async-first**: All I/O operations use async/await with Tokio
- **Testing**: Integration tests separate from unit tests, single-threaded execution