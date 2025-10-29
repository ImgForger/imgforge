#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Constants
IMGFORGE_PORT=3000
PROMETHEUS_PORT=9090
GRAFANA_PORT=3001
METRICS_PORT=9000
DEPLOYMENT_DIR="$HOME/.imgforge"
INSTALL_DIR="/opt/imgforge"
CONFIG_DIR="/etc/imgforge"
CACHE_DIR="/var/lib/imgforge/cache"
LOG_DIR="/var/log/imgforge"
DATA_DIR="/var/lib/imgforge"

# Helper functions
print_header() {
    echo ""
    echo -e "${CYAN}╔═════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}║  ${BLUE}██╗███╗   ███╗ ██████╗ ███████╗ ██████╗ ██████╗  ██████╗ ███████╗${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║████╗ ████║██╔════╝ ██╔════╝██╔═══██╗██╔══██╗██╔════╝ ██╔════╝${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██╔████╔██║██║  ███╗█████╗  ██║   ██║██████╔╝██║  ███╗█████╗${NC}    ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██║╚██╔╝██║██║   ██║██╔══╝  ██║   ██║██╔══██╗██║   ██║██╔══╝${NC}    ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██║ ╚═╝ ██║╚██████╔╝██║     ╚██████╔╝██║  ██║╚██████╔╝███████╗${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}╚═╝╚═╝     ╚═╝ ╚═════╝ ╚═╝      ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚══════╝${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}║              ${GREEN}Fast, Secure Image Transformation Server${NC}               ${CYAN}║${NC}"
    echo -e "${CYAN}║                        ${YELLOW}Systemd Deployment${NC}                           ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}╚═════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Check if running as root
check_root() {
    if [ "$EUID" -eq 0 ]; then
        print_error "This script should not be run as root."
        print_info "Run it as a regular user. It will prompt for sudo when needed."
        exit 1
    fi
}

# Check if port is available
check_port() {
    local port=$1
    local service=$2

    if command -v lsof &> /dev/null; then
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            print_error "Port $port is already in use (required for $service)"
            echo "       Please free up this port and try again."
            echo "       You can check what's using it with: sudo lsof -i :$port"
            return 1
        fi
    elif command -v netstat &> /dev/null; then
        if netstat -tuln | grep -q ":$port "; then
            print_error "Port $port is already in use (required for $service)"
            echo "       Please free up this port and try again."
            return 1
        fi
    elif command -v ss &> /dev/null; then
        if ss -tuln | grep -q ":$port "; then
            print_error "Port $port is already in use (required for $service)"
            echo "       Please free up this port and try again."
            return 1
        fi
    else
        print_warning "Cannot check port availability (lsof, netstat, or ss not found)"
        print_info "Proceeding anyway..."
    fi
    return 0
}

# Detect package manager and OS
detect_system() {
    print_info "Detecting system type..."

    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$NAME
        OS_VERSION=$VERSION_ID
    elif [ -f /etc/redhat-release ]; then
        OS="Red Hat"
        OS_VERSION=$(cat /etc/redhat-release | sed 's/.*release \([0-9]\).*/\1/')
    else
        OS="Unknown"
        OS_VERSION="Unknown"
    fi

    if command -v apt-get &> /dev/null; then
        PKG_MANAGER="apt"
    elif command -v yum &> /dev/null; then
        PKG_MANAGER="yum"
    elif command -v dnf &> /dev/null; then
        PKG_MANAGER="dnf"
    elif command -v pacman &> /dev/null; then
        PKG_MANAGER="pacman"
    else
        print_error "Unsupported package manager"
        exit 1
    fi

    print_success "Detected: $OS (Package manager: $PKG_MANAGER)"
}

# Install system dependencies
install_dependencies() {
    print_info "Installing system dependencies..."

    case $PKG_MANAGER in
        apt)
            sudo apt-get update -qq
            sudo apt-get install -y curl wget ca-certificates
            print_success "Base dependencies installed"
            ;;
        yum|dnf)
            sudo $PKG_MANAGER install -y curl wget ca-certificates
            print_success "Base dependencies installed"
            ;;
        pacman)
            sudo pacman -Sy --noconfirm curl wget ca-certificates
            print_success "Base dependencies installed"
            ;;
    esac
}

# Install libvips
install_libvips() {
    print_info "Checking for libvips..."

    if command -v vips &> /dev/null; then
        VIPS_VERSION=$(vips --version | head -n1)
        print_success "libvips already installed: $VIPS_VERSION"
        return 0
    fi

    print_info "Installing libvips..."

    case $PKG_MANAGER in
        apt)
            sudo apt-get install -y libvips-dev libvips-tools
            ;;
        yum|dnf)
            if [ "$PKG_MANAGER" = "dnf" ]; then
                sudo dnf install -y epel-release || true
                sudo dnf install -y vips vips-devel
            else
                sudo yum install -y epel-release || true
                sudo yum install -y vips vips-devel
            fi
            ;;
        pacman)
            sudo pacman -S --noconfirm libvips
            ;;
    esac

    if command -v vips &> /dev/null; then
        VIPS_VERSION=$(vips --version | head -n1)
        print_success "libvips installed: $VIPS_VERSION"
    else
        print_error "Failed to install libvips"
        print_info "Please install libvips manually and run this script again"
        exit 1
    fi
}

# Get latest release version from GitHub
get_latest_release() {
    local latest_release

    print_info "Fetching latest release information..." >&2

    if command -v curl &> /dev/null; then
        latest_release=$(curl -s https://api.github.com/repos/ImgForger/imgforge/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    elif command -v wget &> /dev/null; then
        latest_release=$(wget -qO- https://api.github.com/repos/ImgForger/imgforge/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    else
        print_error "Neither curl nor wget is available"
        exit 1
    fi

    if [ -z "$latest_release" ]; then
        print_error "Failed to fetch latest release information"
        exit 1
    fi

    echo "$latest_release"
}

# Download and install imgforge binary
download_imgforge() {
    print_info "Downloading imgforge binary..."

    local arch=$(uname -m)
    local binary_arch

    if [ "$arch" = "x86_64" ]; then
        binary_arch="amd64"
    elif [ "$arch" = "aarch64" ] || [ "$arch" = "arm64" ]; then
        binary_arch="arm64"
    else
        print_error "Unsupported architecture: $arch"
        print_info "Supported architectures: x86_64 (amd64), aarch64/arm64"
        exit 1
    fi

    local version=$(get_latest_release)
    local download_url="https://github.com/ImgForger/imgforge/releases/download/${version}/imgforge-linux-${binary_arch}.tar.gz"

    print_info "Latest version: $version"
    print_info "Architecture: $binary_arch"
    print_info "Downloading from: $download_url"

    local tmp_dir=$(mktemp -d)
    cd "$tmp_dir"

    if command -v curl &> /dev/null; then
        if ! curl -L -o imgforge.tar.gz "$download_url"; then
            print_error "Failed to download imgforge binary"
            cd ~
            rm -rf "$tmp_dir"
            exit 1
        fi
    elif command -v wget &> /dev/null; then
        if ! wget -O imgforge.tar.gz "$download_url"; then
            print_error "Failed to download imgforge binary"
            cd ~
            rm -rf "$tmp_dir"
            exit 1
        fi
    fi

    print_info "Extracting binary..."
    if ! tar xzf imgforge.tar.gz; then
        print_error "Failed to extract binary"
        cd ~
        rm -rf "$tmp_dir"
        exit 1
    fi

    print_info "Installing imgforge binary..."
    sudo mkdir -p "$INSTALL_DIR"
    sudo cp imgforge "$INSTALL_DIR/imgforge"
    sudo chmod +x "$INSTALL_DIR/imgforge"

    cd ~
    rm -rf "$tmp_dir"

    print_success "imgforge $version installed to $INSTALL_DIR/imgforge"
}

# Generate secure random hex string
generate_secure_key() {
    if command -v openssl &> /dev/null; then
        openssl rand -hex 64
    else
        head -c 64 /dev/urandom | od -An -tx1 | tr -d ' \n'
    fi
}

# Prompt helper
prompt_read() {
    local prompt_text="$1"
    local result_var="$2"
    local input_value

    if [ -t 0 ]; then
        read -rp "$(echo -e "$prompt_text")" input_value
    elif [ -r /dev/tty ]; then
        read -rp "$(echo -e "$prompt_text")" input_value </dev/tty
    else
        print_error "No interactive terminal available for prompt: $prompt_text"
        exit 1
    fi

    printf -v "$result_var" '%s' "$input_value"
}

# Ask user for cache configuration
ask_cache_config() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}                  Cache Configuration${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "imgforge supports multiple caching strategies to improve performance:"
    echo ""
    echo -e "  ${GREEN}1)${NC} Memory Cache    - Fast in-memory caching (best for smaller workloads)"
    echo -e "  ${GREEN}2)${NC} Disk Cache      - Persistent file-based caching (for larger datasets)"
    echo -e "  ${GREEN}3)${NC} Hybrid Cache    - Combined memory + disk caching (best performance)"
    echo -e "  ${GREEN}4)${NC} No Cache        - Disable caching entirely"
    echo ""

    while true; do
        prompt_read "${CYAN}Choose cache type [1-4]:${NC} " cache_choice
        case $cache_choice in
            1)
                CACHE_TYPE="memory"
                CACHE_MEMORY_CAPACITY="1000"
                print_success "Memory cache selected (1000 entries)"
                break
                ;;
            2)
                CACHE_TYPE="disk"
                CACHE_DISK_PATH="$CACHE_DIR"
                CACHE_DISK_CAPACITY="10737418240"
                print_success "Disk cache selected (10 GB at $CACHE_DIR)"
                break
                ;;
            3)
                CACHE_TYPE="hybrid"
                CACHE_MEMORY_CAPACITY="1000"
                CACHE_DISK_PATH="$CACHE_DIR"
                CACHE_DISK_CAPACITY="10737418240"
                print_success "Hybrid cache selected (1000 entries + 10 GB disk)"
                break
                ;;
            4)
                CACHE_TYPE="none"
                print_success "Cache disabled"
                break
                ;;
            *)
                print_error "Invalid choice. Please enter 1, 2, 3, or 4."
                ;;
        esac
    done
    echo ""
}

# Ask user for monitoring configuration
ask_monitoring_config() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}               Monitoring Configuration${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "imgforge can be monitored with Prometheus and Grafana:"
    echo ""
    echo -e "  • ${GREEN}Prometheus${NC} - Collects and stores metrics from imgforge"
    echo -e "  • ${GREEN}Grafana${NC}    - Provides beautiful dashboards and visualizations"
    echo ""
    echo "This will use the following ports:"
    echo -e "  • Prometheus: ${YELLOW}$PROMETHEUS_PORT${NC}"
    echo -e "  • Grafana:    ${YELLOW}$GRAFANA_PORT${NC} (admin/admin)"
    echo -e "  • Metrics:    ${YELLOW}$METRICS_PORT${NC}"
    echo ""

    while true; do
        prompt_read "${CYAN}Enable Prometheus + Grafana monitoring? [y/N]:${NC} " monitoring_choice
        case $monitoring_choice in
            [Yy]*)
                ENABLE_MONITORING=true
                print_success "Monitoring enabled"
                break
                ;;
            [Nn]*|"")
                ENABLE_MONITORING=false
                print_info "Monitoring disabled (you can enable it later)"
                break
                ;;
            *)
                print_error "Invalid choice. Please enter y or n."
                ;;
        esac
    done
    echo ""
}

# Check required ports
check_required_ports() {
    print_info "Checking required ports..."
    local ports_ok=true

    if ! check_port $IMGFORGE_PORT "imgforge"; then
        ports_ok=false
    fi

    if [ "$ENABLE_MONITORING" = true ]; then
        if ! check_port $PROMETHEUS_PORT "Prometheus"; then
            ports_ok=false
        fi
        if ! check_port $GRAFANA_PORT "Grafana"; then
            ports_ok=false
        fi
        if ! check_port $METRICS_PORT "imgforge metrics"; then
            ports_ok=false
        fi
    fi

    if [ "$ports_ok" = false ]; then
        print_error "One or more required ports are in use"
        exit 1
    fi

    print_success "All required ports are available"
}

# Create directory structure
create_directory_structure() {
    print_info "Creating directory structure..."

    sudo mkdir -p "$INSTALL_DIR"
    sudo mkdir -p "$CONFIG_DIR"
    sudo mkdir -p "$DATA_DIR"
    sudo mkdir -p "$LOG_DIR"

    if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
        sudo mkdir -p "$CACHE_DIR"
    fi

    sudo chown -R "$USER:$USER" "$DATA_DIR"
    sudo chown -R "$USER:$USER" "$LOG_DIR"

    if [ "$ENABLE_MONITORING" = true ]; then
        sudo mkdir -p "$DATA_DIR/prometheus"
        sudo mkdir -p "$DATA_DIR/grafana"
        sudo chown -R "$USER:$USER" "$DATA_DIR/prometheus"
        sudo chown -R "$USER:$USER" "$DATA_DIR/grafana"
    fi

    print_success "Directory structure created"
}

# Generate configuration files
generate_configs() {
    print_info "Generating configuration files..."

    if [ -z "$IMGFORGE_KEY" ]; then
        IMGFORGE_KEY=$(generate_secure_key)
    fi
    if [ -z "$IMGFORGE_SALT" ]; then
        IMGFORGE_SALT=$(generate_secure_key)
    fi

    sudo tee "$CONFIG_DIR/imgforge.env" > /dev/null << EOF
# imgforge Configuration
# Generated on $(date)

# Security Keys (KEEP THESE SECRET!)
IMGFORGE_KEY=$IMGFORGE_KEY
IMGFORGE_SALT=$IMGFORGE_SALT

# Server Configuration
IMGFORGE_BIND=3000
IMGFORGE_LOG_LEVEL=info
IMGFORGE_TIMEOUT=30
IMGFORGE_DOWNLOAD_TIMEOUT=10

# Cache Configuration
EOF

    if [ "$CACHE_TYPE" != "none" ]; then
        echo "IMGFORGE_CACHE_TYPE=$CACHE_TYPE" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null

        if [ "$CACHE_TYPE" = "memory" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
            echo "IMGFORGE_CACHE_MEMORY_CAPACITY=$CACHE_MEMORY_CAPACITY" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
        fi

        if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
            echo "IMGFORGE_CACHE_DISK_PATH=$CACHE_DISK_PATH" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
            echo "IMGFORGE_CACHE_DISK_CAPACITY=$CACHE_DISK_CAPACITY" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
        fi
    fi

    if [ "$ENABLE_MONITORING" = true ]; then
        echo "" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
        echo "# Monitoring Configuration" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
        echo "IMGFORGE_PROMETHEUS_BIND=9000" | sudo tee -a "$CONFIG_DIR/imgforge.env" > /dev/null
    fi

    sudo chmod 600 "$CONFIG_DIR/imgforge.env"

    cp "$CONFIG_DIR/imgforge.env" "$DEPLOYMENT_DIR/imgforge.env.backup"
    chmod 600 "$DEPLOYMENT_DIR/imgforge.env.backup"

    print_success "Configuration files generated"
}

# Create systemd service for imgforge
create_imgforge_service() {
    print_info "Creating imgforge systemd service..."

    sudo tee /etc/systemd/system/imgforge.service > /dev/null << EOF
[Unit]
Description=imgforge - Fast, Secure Image Transformation Server
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=$USER
Group=$USER
EnvironmentFile=$CONFIG_DIR/imgforge.env
ExecStart=$INSTALL_DIR/imgforge
Restart=on-failure
RestartSec=5s
StandardOutput=append:$LOG_DIR/imgforge.log
StandardError=append:$LOG_DIR/imgforge-error.log

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$DATA_DIR $LOG_DIR $CACHE_DIR

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable imgforge.service

    print_success "imgforge service created"
}

# Install and configure Prometheus
install_prometheus() {
    print_info "Installing Prometheus..."

    local prometheus_version="2.48.0"
    local arch=$(uname -m)

    if [ "$arch" = "x86_64" ]; then
        arch="amd64"
    elif [ "$arch" = "aarch64" ]; then
        arch="arm64"
    fi

    local tmp_dir=$(mktemp -d)
    cd "$tmp_dir"

    print_info "Downloading Prometheus $prometheus_version..."
    wget -q https://github.com/prometheus/prometheus/releases/download/v${prometheus_version}/prometheus-${prometheus_version}.linux-${arch}.tar.gz

    tar xzf prometheus-${prometheus_version}.linux-${arch}.tar.gz
    cd prometheus-${prometheus_version}.linux-${arch}

    sudo cp prometheus promtool /usr/local/bin/
    sudo cp -r consoles console_libraries /etc/prometheus/ 2>/dev/null || sudo mkdir -p /etc/prometheus && sudo cp -r consoles console_libraries /etc/prometheus/

    sudo tee /etc/prometheus/prometheus.yml > /dev/null << EOF
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'imgforge'
    static_configs:
      - targets: ['localhost:9000']
    metrics_path: '/metrics'
    scrape_interval: 10s
EOF

    cd ~
    rm -rf "$tmp_dir"

    sudo tee /etc/systemd/system/prometheus.service > /dev/null << EOF
[Unit]
Description=Prometheus Monitoring System
After=network.target

[Service]
Type=simple
User=$USER
Group=$USER
ExecStart=/usr/local/bin/prometheus \\
  --config.file=/etc/prometheus/prometheus.yml \\
  --storage.tsdb.path=$DATA_DIR/prometheus \\
  --web.console.templates=/etc/prometheus/consoles \\
  --web.console.libraries=/etc/prometheus/console_libraries \\
  --web.listen-address=:9090
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable prometheus.service

    print_success "Prometheus installed and configured"
}

# Install and configure Grafana
install_grafana() {
    print_info "Installing Grafana..."

    case $PKG_MANAGER in
        apt)
            sudo apt-get install -y software-properties-common
            sudo wget -q -O /usr/share/keyrings/grafana.key https://apt.grafana.com/gpg.key
            echo "deb [signed-by=/usr/share/keyrings/grafana.key] https://apt.grafana.com stable main" | sudo tee /etc/apt/sources.list.d/grafana.list
            sudo apt-get update -qq
            sudo apt-get install -y grafana
            ;;
        yum|dnf)
            sudo tee /etc/yum.repos.d/grafana.repo > /dev/null << EOF
[grafana]
name=grafana
baseurl=https://rpm.grafana.com
repo_gpgcheck=1
enabled=1
gpgcheck=1
gpgkey=https://rpm.grafana.com/gpg.key
sslverify=1
sslcacert=/etc/pki/tls/certs/ca-bundle.crt
EOF
            sudo $PKG_MANAGER install -y grafana
            ;;
        *)
            print_warning "Grafana automatic installation not supported for this package manager"
            print_info "Installing Grafana from binary..."

            local grafana_version="10.2.2"
            local arch=$(uname -m)

            if [ "$arch" = "x86_64" ]; then
                arch="amd64"
            elif [ "$arch" = "aarch64" ]; then
                arch="arm64"
            fi

            local tmp_dir=$(mktemp -d)
            cd "$tmp_dir"

            wget -q https://dl.grafana.com/oss/release/grafana-${grafana_version}.linux-${arch}.tar.gz
            tar xzf grafana-${grafana_version}.linux-${arch}.tar.gz

            sudo mv grafana-${grafana_version} /opt/grafana
            sudo ln -sf /opt/grafana/bin/grafana-server /usr/local/bin/grafana-server

            cd ~
            rm -rf "$tmp_dir"
            ;;
    esac

    sudo mkdir -p /etc/grafana/provisioning/datasources
    sudo mkdir -p /etc/grafana/provisioning/dashboards

    sudo tee /etc/grafana/provisioning/datasources/prometheus.yml > /dev/null << EOF
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    uid: prometheus
    url: http://localhost:9090
    isDefault: true
    editable: true
EOF

    sudo tee /etc/grafana/provisioning/dashboards/imgforge.yml > /dev/null << EOF
apiVersion: 1

providers:
  - name: 'imgforge'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /etc/grafana/dashboards
      foldersFromFilesStructure: true
EOF

    sudo mkdir -p /etc/grafana/dashboards

    print_info "Downloading imgforge Grafana dashboard..."
    sudo wget -q -O /etc/grafana/dashboards/imgforge-dashboard.json https://raw.githubusercontent.com/ImgForger/imgforge/main/grafana-dashboards/imgforge-dashboard.json 2>/dev/null || print_warning "Could not download dashboard (will need to import manually)"

    sudo tee /etc/grafana/grafana.ini > /dev/null << EOF
[server]
http_port = 3001

[security]
admin_user = admin
admin_password = admin

[users]
allow_sign_up = false
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable grafana-server.service

    print_success "Grafana installed and configured"
}

# Start services
start_services() {
    print_info "Starting services..."

    sudo systemctl start imgforge.service

    if [ "$ENABLE_MONITORING" = true ]; then
        sudo systemctl start prometheus.service
        sudo systemctl start grafana-server.service
    fi

    sleep 3

    if sudo systemctl is-active --quiet imgforge.service; then
        print_success "imgforge service started successfully"
    else
        print_error "Failed to start imgforge service"
        print_info "Check logs with: sudo journalctl -u imgforge.service -n 50"
        exit 1
    fi

    if [ "$ENABLE_MONITORING" = true ]; then
        if sudo systemctl is-active --quiet prometheus.service; then
            print_success "Prometheus service started successfully"
        else
            print_warning "Prometheus service failed to start"
            print_info "Check logs with: sudo journalctl -u prometheus.service -n 50"
        fi

        if sudo systemctl is-active --quiet grafana-server.service; then
            print_success "Grafana service started successfully"
        else
            print_warning "Grafana service failed to start"
            print_info "Check logs with: sudo journalctl -u grafana-server.service -n 50"
        fi
    fi
}

# Health check
health_check() {
    print_info "Performing health check..."

    sleep 2

    if curl -sf http://localhost:$IMGFORGE_PORT/status > /dev/null 2>&1; then
        print_success "imgforge is responding to health checks"
    else
        print_warning "imgforge health check failed"
        print_info "The service might still be starting up. Check logs with: sudo journalctl -u imgforge.service -f"
    fi
}

# Display summary
display_summary() {
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                                                            ║${NC}"
    echo -e "${GREEN}║        ${CYAN}🎉  imgforge Deployment Successful!  🎉${NC}             ${GREEN}║${NC}"
    echo -e "${GREEN}║                                                            ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}Service URLs:${NC}"
    echo -e "  • imgforge API:    http://localhost:$IMGFORGE_PORT"
    echo -e "  • Status endpoint: http://localhost:$IMGFORGE_PORT/status"

    if [ "$ENABLE_MONITORING" = true ]; then
        echo -e "  • Metrics:         http://localhost:$METRICS_PORT/metrics"
        echo -e "  • Prometheus:      http://localhost:$PROMETHEUS_PORT"
        echo -e "  • Grafana:         http://localhost:$GRAFANA_PORT (admin/admin)"
    fi

    echo ""
    echo -e "${BLUE}Service Management:${NC}"
    echo -e "  • Start:    sudo systemctl start imgforge"
    echo -e "  • Stop:     sudo systemctl stop imgforge"
    echo -e "  • Restart:  sudo systemctl restart imgforge"
    echo -e "  • Status:   sudo systemctl status imgforge"
    echo -e "  • Logs:     sudo journalctl -u imgforge -f"

    echo ""
    echo -e "${BLUE}Configuration:${NC}"
    echo -e "  • Config file:  $CONFIG_DIR/imgforge.env"
    echo -e "  • Backup:       $DEPLOYMENT_DIR/imgforge.env.backup"
    echo -e "  • Logs:         $LOG_DIR/"
    echo -e "  • Data:         $DATA_DIR/"

    if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
        echo -e "  • Cache:        $CACHE_DIR/"
    fi

    echo ""
    echo -e "${YELLOW}Important Security Notes:${NC}"
    echo -e "  • Your HMAC keys are stored in: ${YELLOW}$CONFIG_DIR/imgforge.env${NC}"
    echo -e "  • Backup this file securely!"
    echo -e "  • A backup copy is also saved in: ${YELLOW}$DEPLOYMENT_DIR/imgforge.env.backup${NC}"

    if [ "$ENABLE_MONITORING" = true ]; then
        echo -e "  • Change Grafana password (currently admin/admin) after first login"
    fi

    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Main execution
main() {
    # print_header

    # detect_system

    # mkdir -p "$DEPLOYMENT_DIR"

    # ask_cache_config
    # ask_monitoring_config
    # check_required_ports

    # print_info "Installing dependencies..."
    # install_dependencies
    # install_libvips

    # download_imgforge

    # create_directory_structure
    # generate_configs
    # create_imgforge_service

    # if [ "$ENABLE_MONITORING" = true ]; then
    #     install_prometheus
    #     install_grafana
    # fi

    # start_services
    # health_check
    display_summary
}

main "$@"
