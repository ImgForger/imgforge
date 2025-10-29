# Deployment Folder Summary

This folder contains everything needed to deploy imgforge on a fresh Linux machine with minimal effort.

## Files Overview

### `deploy.sh` (Main Deployment Script)
**Purpose:** Automated one-line deployment of imgforge  
**Size:** ~24KB  
**Executable:** Yes  

**Features:**
- ✅ Automatic Docker installation and setup
- ✅ Interactive cache configuration (Memory/Disk/Hybrid/None)
- ✅ Optional Prometheus + Grafana monitoring setup
- ✅ Automatic security key generation (128-char hex)
- ✅ Port conflict detection and validation
- ✅ Service health checks and validation
- ✅ Pre-built Grafana dashboard auto-download
- ✅ Beautiful ASCII art interface with colored output
- ✅ Comprehensive error handling and recovery
- ✅ Docker Compose orchestration
- ✅ Automatic service startup and monitoring

**Error Scenarios Handled:**
- Docker not installed → Installs automatically via `get.docker.com`
- Docker daemon not running → Starts and enables Docker service
- Ports already in use → Detects and reports conflicts
- Image pull failures → Reports network/registry issues
- Service startup failures → Shows logs and debugging info
- libvips missing → Informational only (runs in container)

**Usage:**
```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh | bash
```

### `uninstall.sh` (Cleanup Script)
**Purpose:** Complete removal of imgforge and all data  
**Size:** ~7KB  
**Executable:** Yes  

**Features:**
- ✅ Interactive confirmation before removal
- ✅ Stops and removes all containers
- ✅ Removes Docker volumes (prometheus-data, grafana-data)
- ✅ Removes configuration files (~/.imgforge)
- ✅ Removes cache directory (/var/imgforge)
- ✅ Optional Docker image removal
- ✅ Comprehensive cleanup across all naming variations

**Usage:**
```bash
./uninstall.sh
```

### `README.md` (User Documentation)
**Purpose:** Complete deployment guide for end users  
**Size:** ~8KB  

**Contents:**
- Quick start instructions
- Configuration options explained
- Default settings reference
- Post-deployment steps
- Security considerations
- Troubleshooting guide
- Manual configuration examples
- Uninstall instructions
- System requirements

**Target Audience:** System administrators, DevOps engineers, developers

### `TESTING.md` (Developer Guide)
**Purpose:** Testing and validation guide for contributors  
**Size:** ~8KB  

**Contents:**
- Syntax validation commands
- Dry-run testing procedures
- VM testing with Vagrant
- Container testing with Docker
- Manual testing checklist
- Automated test script examples
- Error scenario testing
- CI/CD integration examples
- Distribution compatibility matrix
- Issue reporting guidelines

**Target Audience:** Contributors, QA engineers, maintainers

### `.gitignore`
**Purpose:** Prevent test artifacts from being committed  

**Excludes:**
- `test-deployment/` - Local test directories
- `*.test` - Test output files

## Generated Files (After Deployment)

When the deployment script runs, it creates the following structure in `~/.imgforge/`:

```
~/.imgforge/
├── .env                             # Environment variables with secrets
├── docker-compose.yml               # Service orchestration
├── prometheus/
│   └── prometheus.yml              # Scrape configuration
├── grafana-dashboards/
│   ├── dashboard-provisioning.yml  # Dashboard auto-provisioning config
│   └── imgforge-dashboard.json     # Pre-built imgforge dashboard (downloaded)
└── grafana-datasources.yml         # Grafana → Prometheus connection
```

### `.env` File Structure
```bash
# Security Keys (Auto-generated)
IMGFORGE_KEY=<128-char-hex>
IMGFORGE_SALT=<128-char-hex>

# Server Configuration
IMGFORGE_BIND=3000
IMGFORGE_LOG_LEVEL=info
IMGFORGE_TIMEOUT=30
IMGFORGE_DOWNLOAD_TIMEOUT=10

# Cache Configuration (if enabled)
IMGFORGE_CACHE_TYPE=memory|disk|hybrid
IMGFORGE_CACHE_MEMORY_CAPACITY=1000
IMGFORGE_CACHE_DISK_PATH=/var/imgforge/cache
IMGFORGE_CACHE_DISK_CAPACITY=10737418240

# Monitoring Configuration (if enabled)
IMGFORGE_PROMETHEUS_BIND=9000
```

### `docker-compose.yml` Structure

**Minimal (No Monitoring):**
- imgforge service only
- Port 3000 exposed

**Full Stack (With Monitoring):**
- imgforge service (ports 3000, 9000)
- prometheus service (port 9090)
- grafana service (port 3001)
- Custom bridge network
- Persistent volumes

## Configuration Options

### Cache Types

| Type    | Description                    | Default Capacity | Persistence |
|---------|--------------------------------|------------------|-------------|
| Memory  | Fast in-memory cache          | 1000 entries     | No          |
| Disk    | File-based persistent cache   | 10 GB            | Yes         |
| Hybrid  | Memory + Disk combined        | 1000 + 10 GB     | Partial     |
| None    | No caching                    | -                | -           |

### Monitoring Stack

| Service    | Port | Credentials | Purpose                    |
|------------|------|-------------|----------------------------|
| imgforge   | 3000 | -           | Image processing API       |
| Metrics    | 9000 | -           | Prometheus metrics         |
| Prometheus | 9090 | -           | Metrics storage/queries    |
| Grafana    | 3001 | admin/admin | Visualization dashboards   |

## Security Features

### Generated Keys
- 128-character hexadecimal strings
- Cryptographically secure random generation via OpenSSL
- Automatically saved to `.env` file
- Used for HMAC URL signing

### Security Best Practices Implemented
- ✅ Secure key generation (openssl rand -hex 64)
- ✅ Keys stored in protected `.env` file
- ✅ No default/hardcoded keys in deployment
- ✅ HMAC-signed URLs required by default
- ⚠️ Warning displayed about keeping `.env` secure
- ⚠️ Reminder to change Grafana password

### What's NOT Included (User Responsibility)
- HTTPS/TLS termination (use reverse proxy)
- Firewall configuration
- Rate limiting at proxy level
- Bearer token authentication
- Network isolation
- Backup automation

## Port Usage

| Port | Service                 | Optional | Configurable |
|------|------------------------|----------|--------------|
| 3000 | imgforge HTTP API      | No       | Yes          |
| 9000 | imgforge Metrics       | Yes      | Yes          |
| 9090 | Prometheus UI          | Yes      | Yes          |
| 3001 | Grafana UI             | Yes      | Yes          |

## System Requirements

### Minimum
- **OS:** Any modern Linux distribution
- **RAM:** 2 GB
- **Disk:** 5 GB free space
- **Network:** Internet connection for initial setup
- **Tools:** curl or wget

### Recommended
- **OS:** Ubuntu 22.04 LTS or Debian 12
- **RAM:** 4 GB
- **Disk:** 20 GB free space (especially for disk cache)
- **CPU:** 2+ cores
- **Tools:** Docker, curl, lsof/netstat

## Tested Distributions

- ✅ Ubuntu 20.04 LTS
- ✅ Ubuntu 22.04 LTS
- ✅ Ubuntu 24.04 LTS
- ✅ Debian 11 (Bullseye)
- ✅ Debian 12 (Bookworm)
- ⚠️ CentOS 8 Stream (should work)
- ⚠️ Rocky Linux 8/9 (should work)
- ⚠️ Amazon Linux 2/2023 (should work)

## Common Use Cases

### 1. Development Environment
```bash
# Quick setup with memory cache, no monitoring
./deploy.sh
# Choose: 1 (Memory Cache), n (No monitoring)
```

### 2. Staging/Testing
```bash
# Hybrid cache with monitoring
./deploy.sh
# Choose: 3 (Hybrid Cache), y (Enable monitoring)
```

### 3. Production
```bash
# Run deployment script
./deploy.sh
# Choose: 3 (Hybrid Cache), y (Enable monitoring)

# Then configure:
# - Reverse proxy (nginx/Caddy) with HTTPS
# - Firewall rules
# - Backup automation
# - Log aggregation
# - Change Grafana password
```

## Deployment Time

| Configuration        | Time to Deploy | Notes                        |
|---------------------|----------------|------------------------------|
| No Docker installed | 5-10 minutes   | Includes Docker installation |
| Docker installed    | 2-3 minutes    | Just image pull and setup    |
| Minimal (no cache)  | 1-2 minutes    | Fastest option               |
| With monitoring     | 3-5 minutes    | Additional image pulls       |

## Success Indicators

After successful deployment, you should see:

```
╔════════════════════════════════════════════════════════════╗
║                                                            ║
║        🎉  imgforge Deployment Successful!  🎉             ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝

Service Information:
  • imgforge:      http://localhost:3000
  • Health check:  http://localhost:3000/status
  • System info:   http://localhost:3000/info
  ...
```

### Verification Commands
```bash
# Check service
curl http://localhost:3000/status

# Check containers
docker ps | grep imgforge

# Check logs
docker logs imgforge -f

# Check metrics (if monitoring enabled)
curl http://localhost:9000/metrics
```

## Maintenance Commands

```bash
# View logs
docker logs imgforge -f

# Restart service
cd ~/.imgforge && docker compose restart

# Stop service
cd ~/.imgforge && docker compose down

# Update to latest version
cd ~/.imgforge
docker compose pull
docker compose up -d

# Check resource usage
docker stats imgforge
```

## Integration Points

The deployment script is designed to integrate with:

- **CI/CD Pipelines** - Can be called non-interactively with input piping
- **Configuration Management** - Ansible, Chef, Puppet compatible
- **Infrastructure as Code** - Terraform, CloudFormation friendly
- **Container Orchestration** - Can coexist with Kubernetes (for testing)
- **Monitoring Systems** - Prometheus-compatible metrics
- **Logging Aggregation** - JSON structured logs to stdout

## Future Enhancements

Potential improvements for future versions:

- [ ] Non-interactive mode with command-line flags
- [ ] Custom domain/port configuration prompts
- [ ] SSL/TLS certificate integration (Let's Encrypt)
- [ ] Backup/restore functionality
- [ ] Multi-instance deployment support
- [ ] Cloud provider specific variants (AWS, GCP, Azure)
- [ ] Kubernetes deployment option
- [ ] Environment-specific presets (dev, staging, prod)
- [ ] Configuration validation and testing mode
- [ ] Automatic update checks and notifications

## Contributing

To improve the deployment scripts:

1. Test changes on multiple distributions
2. Verify syntax with `bash -n deploy.sh`
3. Run through the testing checklist in `TESTING.md`
4. Update this summary if adding features
5. Follow existing code style and error handling patterns

## Support

- **Documentation:** [deployment/README.md](README.md)
- **Testing Guide:** [deployment/TESTING.md](TESTING.md)
- **Main Project:** https://github.com/ImgForger/imgforge
- **Issues:** https://github.com/ImgForger/imgforge/issues

## License

Same as main imgforge project - see [LICENSE](../LICENSE)
