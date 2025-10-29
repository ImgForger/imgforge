# imgforge Deployment Guide

This directory contains automated deployment scripts that will get imgforge up and running on a fresh Linux machine with minimal effort.

## Available Scripts

### Docker Deployment
- `deploy.sh` - Deploy imgforge using Docker Compose
- `uninstall.sh` - Remove Docker-based deployment

### Systemd Deployment
- `deploy-systemd.sh` - Deploy imgforge as a native systemd service
- `upgrade-systemd.sh` - Upgrade imgforge to the latest version
- `uninstall-systemd.sh` - Remove systemd-based deployment

## Deployment Options

Choose the deployment method that best fits your infrastructure:

### Option 1: Docker Deployment (Recommended)
Uses Docker and Docker Compose for containerized deployment. Simplest and most portable.

### Option 2: Systemd Deployment
Installs imgforge as a native systemd service. Best for environments where Docker is not available or not desired.

---

## Docker Deployment (Option 1)

### Quick Start

Run the deployment script:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh | bash
```

Or download and run locally:

```bash
wget https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh
chmod +x deploy.sh
./deploy.sh
```

### What the Docker Script Does

The Docker deployment script will:

1. **Check Prerequisites**
   - Verify Docker installation
   - Install Docker automatically if not present
   - Check for available ports
   - Validate system requirements

2. **Interactive Configuration**
   - Ask you to choose a caching strategy (Memory, Disk, Hybrid, or None)
   - Ask if you want to enable Prometheus + Grafana monitoring

3. **Automated Setup**
   - Pull the latest imgforge Docker image from `ghcr.io/imgforger/imgforge:latest`
   - Generate secure random keys for HMAC signing
   - Create configuration files with sane defaults
   - Set up monitoring stack (if enabled)
   - Download pre-built Grafana dashboard
   - Start all services using Docker Compose

4. **Health Checks**
   - Verify services are running correctly
   - Check imgforge responds to status endpoint
   - Display access URLs and credentials

### What's NOT Included (User Responsibility)
- HTTPS/TLS termination (use reverse proxy)
- Firewall configuration
- Rate limiting at proxy level
- Bearer token authentication
- Network isolation
- Backup automation

## Configuration Options

### Cache Types

The script offers four caching strategies:

| Type   | Description                 | Default Capacity | Persistence |
|--------|-----------------------------|------------------|-------------|
| Memory | Fast in-memory cache        | 1000 entries     | No          |
| Disk   | File-based persistent cache | 10 GB            | Yes         |
| Hybrid | Memory + Disk combined      | 1000 + 10 GB     | Partial     |
| None   | No caching                  | -                | -           |

### Monitoring

If enabled, the script deploys:

| Service    | Port | Credentials | Purpose                    |
|------------|------|-------------|----------------------------|
| imgforge   | 3000 | -           | Image processing API       |
| Metrics    | 9000 | -           | Prometheus metrics         |
| Prometheus | 9090 | -           | Metrics storage/queries    |
| Grafana    | 3001 | admin/admin | Visualization dashboards   |

## Default Configuration

The script creates a deployment with these defaults:

```bash
# Port Configuration
IMGFORGE_PORT=3000          # Main HTTP service
PROMETHEUS_PORT=9090        # Prometheus UI (if enabled)
GRAFANA_PORT=3001          # Grafana UI (if enabled)
METRICS_PORT=9000          # Metrics endpoint (if enabled)

# Logging
IMGFORGE_LOG_LEVEL=info

# Timeouts
IMGFORGE_TIMEOUT=30
IMGFORGE_DOWNLOAD_TIMEOUT=10

# Security (generated automatically)
IMGFORGE_KEY=<random_128_char_hex>
IMGFORGE_SALT=<random_128_char_hex>
```

## Deployment Location

All configuration files are stored in `~/.imgforge/`:

```
~/.imgforge/
├── .env                          # Environment variables
├── docker-compose.yml            # Docker Compose configuration
├── prometheus/
│   └── prometheus.yml           # Prometheus scrape config
├── grafana-dashboards/
│   ├── dashboard-provisioning.yml  # Dashboard auto-provisioning config
│   └── imgforge-dashboard.json     # Pre-built imgforge dashboard
└── grafana-datasources.yml      # Grafana Prometheus datasource
```

## Error Handling

The script checks for common issues:

- **Port conflicts** - Validates all required ports are available
- **Docker installation** - Installs Docker if missing
- **Docker daemon** - Starts Docker if not running
- **Image pull failures** - Reports network/registry issues
- **Service startup failures** - Shows logs for debugging

## Post-Deployment

After successful deployment:

1. **Test the service:**
   ```bash
   curl http://localhost:3000/status
   ```

2. **View logs:**
   ```bash
   docker logs imgforge -f
   ```

3. **Access monitoring** (if enabled):
   - Prometheus: http://localhost:9090
   - Grafana: http://localhost:3001 (admin/admin)
     - The imgforge dashboard is automatically provisioned and ready to use
     - Look for "ImgForge Monitoring Dashboard" in the dashboard list

4. **Manage the service:**
   ```bash
   cd ~/.imgforge
   
   # Restart services
   docker compose restart
   
   # Stop services
   docker compose down
   
   # Update to latest version
   docker compose pull
   docker compose up -d
   
   # View all logs
   docker compose logs -f
   ```

## Security Considerations

⚠️ **Important Security Notes:**

1. **HMAC Keys**: The script generates secure random keys stored in `~/.imgforge/.env`. Keep this file secure!

2. **Signed URLs**: By default, imgforge requires HMAC-signed URLs for security. Use the generated `IMGFORGE_KEY` and `IMGFORGE_SALT` in the `.env` to make signatures.

3. **Grafana Password**: If monitoring is enabled, change the default Grafana password (admin/admin) immediately after the first login.

4. **Production Deployment**: For production use, consider:
   - Using HTTPS with a reverse proxy (nginx, Caddy, Traefik)
   - Setting up firewall rules
   - Implementing rate limiting at the proxy level
   - Backing up the `.env` file securely
   - Using Docker secrets or environment variable management

## Manual Configuration

To customize the deployment, edit the generated files:

```bash
cd ~/.imgforge

# Edit environment variables
nano .env

# Edit Docker Compose configuration
nano docker-compose.yml

# Apply changes
docker compose up -d
```

### Common Customizations

**Change the port:**
```bash
# In .env
IMGFORGE_BIND=8080

# In docker-compose.yml, update the ports section
```

**Add rate limiting:**
```bash
# In .env
IMGFORGE_RATE_LIMIT_PER_MINUTE=100
```

**Increase cache size:**
```bash
# In .env
IMGFORGE_CACHE_MEMORY_CAPACITY=5000
IMGFORGE_CACHE_DISK_CAPACITY=53687091200  # 50 GB
```

**Enable unsigned URLs (NOT RECOMMENDED for production):**
```bash
# In .env
IMGFORGE_ALLOW_UNSIGNED=true
```

## Troubleshooting

### Service won't start

```bash
# Check logs
docker logs imgforge

# Check if ports are in use
sudo lsof -i :3000

# Verify Docker is running
docker ps
```

### Can't pull Docker image

```bash
# Test connectivity to GitHub Container Registry
docker pull ghcr.io/imgforger/imgforge:latest

# If behind a proxy, configure Docker proxy settings
```

### Cache not working

```bash
# Check cache configuration in .env
cat ~/.imgforge/.env | grep CACHE

# For disk cache, verify directory permissions
ls -la /var/imgforge/cache

# Check metrics
curl http://localhost:9000/metrics | grep cache
```

### Monitoring not showing data

```bash
# Check Prometheus targets
curl http://localhost:9090/targets

# Verify imgforge metrics endpoint
curl http://localhost:9000/metrics

# Check Grafana logs
docker logs imgforge-grafana
```

## Uninstalling

### Automated Uninstall

Use the provided uninstall script for easy removal:

```bash
cd ~/.imgforge
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/uninstall.sh | bash
```

Or if you have the script locally:

```bash
./uninstall.sh
```

The script will:
- Stop and remove all imgforge containers
- Remove Docker volumes
- Remove configuration files
- Remove cache directories
- Optionally remove Docker images

### Manual Uninstall

To manually remove imgforge:

```bash
# Stop and remove containers
cd ~/.imgforge
docker compose down -v

# Remove deployment files
rm -rf ~/.imgforge

# Remove cache directory (if using disk cache)
sudo rm -rf /var/imgforge

# Remove Docker images (optional)
docker rmi ghcr.io/imgforger/imgforge:latest
docker rmi prom/prometheus:latest
docker rmi grafana/grafana:latest
```

### Docker Requirements

- Linux distribution (Ubuntu, Debian, CentOS, RHEL, etc.)
- 2 GB RAM minimum (4 GB recommended)
- 10 GB free disk space (for caching)
- Internet connection (for Docker image download)
- `curl` or `wget` (for running the script)

---

## Systemd Deployment (Option 2)

For environments where Docker is not available or you prefer native system services, use the systemd deployment method. This downloads pre-compiled imgforge binaries and installs them as a systemd service.

### Quick Start

Run the systemd deployment script:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy-systemd.sh | bash
```

Or download and run locally:

```bash
wget https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy-systemd.sh
chmod +x deploy-systemd.sh
./deploy-systemd.sh
```

### What the Systemd Script Does

The systemd deployment script will:

1. **Check Prerequisites and Install Dependencies**
   - Detect your Linux distribution and package manager
   - Install essential tools (curl, wget, ca-certificates)
   - Install libvips and libvips-tools (required for image processing)
   - Verify all required ports are available

2. **Download imgforge Binary**
   - Fetch the latest release information from GitHub
   - Detect system architecture (amd64 or arm64)
   - Download the pre-compiled binary for your architecture
   - Extract and install the binary to `/opt/imgforge/`

3. **Interactive Configuration**
   - Ask you to choose a caching strategy (Memory, Disk, Hybrid, or None)
   - Ask if you want to enable Prometheus + Grafana monitoring

4. **System Setup**
   - Create systemd service files for imgforge
   - Generate secure random keys for HMAC signing
   - Create configuration files with sane defaults
   - Set up proper directory structure and permissions
   - Install and configure Prometheus (if monitoring enabled)
   - Install and configure Grafana (if monitoring enabled)

5. **Service Management**
   - Enable services to start on boot
   - Start all services
   - Perform health checks
   - Display access URLs and credentials

### System Locations

The systemd deployment uses standard Linux filesystem locations:

```
/opt/imgforge/               # Binary installation
├── imgforge                 # Main executable

/etc/imgforge/               # Configuration
├── imgforge.env             # Environment variables and keys

/var/lib/imgforge/           # Data and state
├── cache/                   # Disk cache (if enabled)
├── prometheus/              # Prometheus data (if enabled)
└── grafana/                 # Grafana data (if enabled)

/var/log/imgforge/           # Logs
├── imgforge.log             # Application logs
└── imgforge-error.log       # Error logs

~/.imgforge/                 # User backup
└── imgforge.env.backup      # Configuration backup
```

### Service Management

After deployment, manage imgforge using standard systemd commands:

```bash
# Start imgforge
sudo systemctl start imgforge

# Stop imgforge
sudo systemctl stop imgforge

# Restart imgforge
sudo systemctl restart imgforge

# Check status
sudo systemctl status imgforge

# View logs (live)
sudo journalctl -u imgforge -f

# View last 100 log lines
sudo journalctl -u imgforge -n 100

# Enable auto-start on boot (enabled by default)
sudo systemctl enable imgforge

# Disable auto-start
sudo systemctl disable imgforge
```

If monitoring is enabled:

```bash
# Manage Prometheus
sudo systemctl start/stop/restart prometheus
sudo journalctl -u prometheus -f

# Manage Grafana
sudo systemctl start/stop/restart grafana-server
sudo journalctl -u grafana-server -f
```

### Configuration Changes

To modify the configuration after deployment:

1. Edit the environment file:
   ```bash
   sudo nano /etc/imgforge/imgforge.env
   ```

2. Restart the service to apply changes:
   ```bash
   sudo systemctl restart imgforge
   ```

### Common Configuration Changes

**Change the port:**
```bash
# In /etc/imgforge/imgforge.env
IMGFORGE_BIND=8080

# Restart the service
sudo systemctl restart imgforge
```

**Add rate limiting:**
```bash
# In /etc/imgforge/imgforge.env
IMGFORGE_RATE_LIMIT_PER_MINUTE=100

# Restart the service
sudo systemctl restart imgforge
```

**Increase cache size:**
```bash
# In /etc/imgforge/imgforge.env
IMGFORGE_CACHE_MEMORY_CAPACITY=5000
IMGFORGE_CACHE_DISK_CAPACITY=53687091200  # 50 GB

# Restart the service
sudo systemctl restart imgforge
```

**Enable unsigned URLs (NOT RECOMMENDED for production):**
```bash
# In /etc/imgforge/imgforge.env
IMGFORGE_ALLOW_UNSIGNED=true

# Restart the service
sudo systemctl restart imgforge
```

### Updating imgforge

#### Automated Upgrade (Recommended)

Use the provided upgrade script for safe, automated updates with automatic rollback on failure:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/upgrade-systemd.sh | bash
```

Or if you have the script locally:

```bash
cd /path/to/deployment
./upgrade-systemd.sh
```

The upgrade script will:
- Check for available updates
- Create a backup of the current binary
- Download and install the latest version
- Verify the service starts correctly
- Perform health checks
- Automatically rollback if anything fails

#### Manual Upgrade

To manually update to the latest version:

```bash
# Stop the service
sudo systemctl stop imgforge

# Download the latest version
cd /tmp
LATEST_VERSION=$(curl -s https://api.github.com/repos/ImgForger/imgforge/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
ARCH=$(uname -m)

# Determine architecture
if [ "$ARCH" = "x86_64" ]; then
    BINARY_ARCH="amd64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    BINARY_ARCH="arm64"
fi

# Download and extract
curl -L -o imgforge.tar.gz "https://github.com/ImgForger/imgforge/releases/download/${LATEST_VERSION}/imgforge-linux-${BINARY_ARCH}.tar.gz"
tar xzf imgforge.tar.gz

# Backup current binary (optional but recommended)
sudo cp /opt/imgforge/imgforge /opt/imgforge/imgforge.backup

# Replace the binary
sudo cp imgforge /opt/imgforge/imgforge
sudo chmod +x /opt/imgforge/imgforge

# Cleanup
rm -f imgforge imgforge.tar.gz

# Start the service
sudo systemctl start imgforge

# Verify
sudo systemctl status imgforge
curl http://localhost:3000/status
```

### Monitoring Setup

When monitoring is enabled, the script:

1. **Installs Prometheus**
   - Downloads and installs the latest Prometheus binary
   - Configures it to scrape metrics from imgforge
   - Sets up as a systemd service
   - Accessible at http://localhost:9090

2. **Installs Grafana**
   - Installs via official repository (apt/yum/dnf)
   - Falls back to binary installation if needed
   - Configures Prometheus as datasource
   - Attempts to download the imgforge dashboard
   - Accessible at http://localhost:3001 (admin/admin)

### Troubleshooting

#### Service won't start

```bash
# Check service status
sudo systemctl status imgforge

# View detailed logs
sudo journalctl -u imgforge -n 100 --no-pager

# Check configuration
cat /etc/imgforge/imgforge.env

# Verify binary exists and is executable
ls -la /opt/imgforge/imgforge

# Test binary manually
sudo -u $USER /opt/imgforge/imgforge
```

#### Port already in use

```bash
# Check what's using the port
sudo lsof -i :3000

# Or use netstat
sudo netstat -tulpn | grep :3000

# Change port in configuration
sudo nano /etc/imgforge/imgforge.env
# Update IMGFORGE_BIND=3000 to a different port

sudo systemctl restart imgforge
```

#### Download or installation failures

```bash
# Ensure libvips is installed
vips --version

# Install libvips manually if needed
# Ubuntu/Debian:
sudo apt-get install libvips-tools

# RHEL/CentOS/Fedora:
sudo dnf install vips

# Check internet connectivity
curl -I https://github.com

# Verify architecture is supported (should be x86_64 or aarch64)
uname -m

# Try downloading manually
LATEST_VERSION=$(curl -s https://api.github.com/repos/ImgForger/imgforge/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
echo "Latest version: $LATEST_VERSION"

# Check available releases
curl -s https://api.github.com/repos/ImgForger/imgforge/releases/latest | grep browser_download_url
```

#### Cache not working

```bash
# Check cache configuration
grep CACHE /etc/imgforge/imgforge.env

# For disk cache, verify directory exists and has correct permissions
ls -la /var/lib/imgforge/cache

# Check metrics to see cache stats
curl http://localhost:9000/metrics | grep cache
```

#### Monitoring not showing data

```bash
# Check Prometheus is running and scraping
sudo systemctl status prometheus
curl http://localhost:9090/targets

# Verify imgforge metrics endpoint
curl http://localhost:9000/metrics

# Check Grafana
sudo systemctl status grafana-server
curl http://localhost:3001
```

### Uninstalling

Use the provided uninstall script for systemd deployments:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/uninstall-systemd.sh | bash
```

Or if you have it locally:

```bash
./uninstall-systemd.sh
```

The script will:
- Stop and disable all services
- Remove systemd service files
- Remove binaries
- Optionally remove configuration files
- Optionally remove data and cache
- Optionally uninstall Grafana package

#### Manual Uninstall

To manually remove imgforge:

```bash
# Stop and disable services
sudo systemctl stop imgforge prometheus grafana-server
sudo systemctl disable imgforge prometheus grafana-server

# Remove service files
sudo rm /etc/systemd/system/imgforge.service
sudo rm /etc/systemd/system/prometheus.service
sudo systemctl daemon-reload

# Remove binaries
sudo rm -rf /opt/imgforge
sudo rm /usr/local/bin/prometheus
sudo rm /usr/local/bin/promtool

# Remove configuration
sudo rm -rf /etc/imgforge
sudo rm -rf /etc/prometheus

# Remove data (be careful - this removes cache and logs!)
sudo rm -rf /var/lib/imgforge
sudo rm -rf /var/log/imgforge

# Remove user backup
rm -rf ~/.imgforge

# Optionally remove Grafana
# Ubuntu/Debian:
sudo apt-get remove grafana

# RHEL/CentOS/Fedora:
sudo dnf remove grafana
```

### Systemd Requirements

- Linux distribution with systemd (Ubuntu 16.04+, Debian 8+, CentOS 7+, RHEL 7+, Fedora, etc.)
- Architecture: x86_64 (amd64) or aarch64/arm64
- 2 GB RAM minimum (4 GB recommended)
- 10 GB free disk space minimum (for caching and monitoring data)
- Internet connection (for downloading binaries and dependencies)
- `curl` or `wget` (for running the script)
- Sufficient privileges to use `sudo`
- libvips will be installed automatically

---

## Comparison: Docker vs Systemd

| Feature                | Docker Deployment                          | Systemd Deployment                 |
|------------------------|--------------------------------------------|------------------------------------|
| **Installation Time**  | Fast (~2-5 minutes)                        | Fast (~3-7 minutes)                |
| **Disk Space**         | ~500 MB (images)                           | ~100 MB (binary + libs)            |
| **Dependencies**       | Docker only                                | libvips only                       |
| **Updates**            | `docker compose pull`                      | Download new binary                |
| **Isolation**          | Container isolation                        | System process                     |
| **Portability**        | High (same image everywhere)               | Medium (compiled per system)       |
| **Resource Overhead**  | Small (container overhead)                 | None (native process)              |
| **Log Management**     | Docker logs                                | systemd journald                   |
| **Service Management** | docker compose                             | systemctl                          |
| **Best For**           | Quick deployment, container infrastructure | Bare metal, no Docker environments |

## Support

- **Documentation**: https://imgforger.github.io/
- **Issues**: https://github.com/ImgForger/imgforge/issues
- **Contributing**: See [CONTRIBUTING.md](../CONTRIBUTING.md)
