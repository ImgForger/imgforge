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
IMGFORGE_IMAGE="ghcr.io/imgforger/imgforge:latest"
IMGFORGE_PORT=3000
PROMETHEUS_PORT=9090
GRAFANA_PORT=3001
METRICS_PORT=9000
DEPLOYMENT_DIR="$HOME/.imgforge"

# Helper functions
print_header() {
    echo ""
    echo -e "${CYAN}╔═════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                                                             ║${NC}"
    echo -e "${CYAN}║  ${BLUE}██╗███╗   ███╗ ██████╗ ███████╗ ██████╗ ██████╗  ██████╗ ███████╗${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║████╗ ████║██╔════╝ ██╔════╝██╔═══██╗██╔══██╗██╔════╝ ██╔════╝${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██╔████╔██║██║  ███╗█████╗  ██║   ██║██████╔╝██║  ███╗█████╗${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██║╚██╔╝██║██║   ██║██╔══╝  ██║   ██║██╔══██╗██║   ██║██╔══╝${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}██║██║ ╚═╝ ██║╚██████╔╝██║     ╚██████╔╝██║  ██║╚██████╔╝███████╗${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║  ${BLUE}╚═╝╚═╝     ╚═╝ ╚═════╝ ╚═╝      ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚══════╝${NC}  ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                             ║${NC}"
    echo -e "${CYAN}║         ${GREEN}Fast, Secure Image Transformation Server${NC}          ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                             ║${NC}"
    echo -e "${CYAN}╚═════════════════════════════════════════════════════════════╝${NC}"
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
        print_warning "Running as root. This is not recommended for Docker operations."
        print_info "Consider running without sudo and adding your user to the docker group."
        echo ""
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

# Check and install Docker
check_docker() {
    print_info "Checking Docker installation..."
    
    if command -v docker &> /dev/null; then
        DOCKER_VERSION=$(docker --version | cut -d ' ' -f3 | cut -d ',' -f1)
        print_success "Docker is already installed (version $DOCKER_VERSION)"
        
        # Check if Docker daemon is running
        if ! docker ps &> /dev/null; then
            print_error "Docker daemon is not running"
            print_info "Attempting to start Docker..."
            
            if command -v systemctl &> /dev/null; then
                sudo systemctl start docker
                sudo systemctl enable docker
                sleep 2
                
                if docker ps &> /dev/null; then
                    print_success "Docker daemon started successfully"
                else
                    print_error "Failed to start Docker daemon. Please start it manually."
                    exit 1
                fi
            else
                print_error "Cannot start Docker automatically. Please start it manually."
                exit 1
            fi
        fi
    else
        print_info "Docker not found. Installing Docker..."
        
        # Download and run Docker installation script
        if ! curl -fsSL https://get.docker.com -o /tmp/get-docker.sh; then
            print_error "Failed to download Docker installation script"
            exit 1
        fi
        
        print_info "Running Docker installation script (this may take a few minutes)..."
        if sudo sh /tmp/get-docker.sh; then
            print_success "Docker installed successfully"
            rm /tmp/get-docker.sh
            
            # Add current user to docker group if not root
            if [ "$EUID" -ne 0 ]; then
                print_info "Adding current user to docker group..."
                sudo usermod -aG docker "$USER"
                print_warning "You may need to log out and back in for group changes to take effect"
                print_info "Or run: newgrp docker"
            fi
        else
            print_error "Docker installation failed"
            rm -f /tmp/get-docker.sh
            exit 1
        fi
    fi
    
    # Check Docker Compose
    if ! docker compose version &> /dev/null; then
        print_warning "Docker Compose plugin not found"
        print_info "Installing Docker Compose plugin..."
        
        # Docker Compose should come with Docker, but try to install if missing
        if command -v apt-get &> /dev/null; then
            sudo apt-get update -qq
            sudo apt-get install -y docker-compose-plugin
        elif command -v yum &> /dev/null; then
            sudo yum install -y docker-compose-plugin
        else
            print_error "Cannot install Docker Compose automatically on this system"
            print_info "Please install Docker Compose manually: https://docs.docker.com/compose/install/"
            exit 1
        fi
    fi
    
    print_success "Docker Compose is available"
}

# Check for libvips (informational only, as it's in the container)
check_libvips() {
    print_info "Checking for libvips (optional for local builds)..."
    
    if command -v vips &> /dev/null; then
        VIPS_VERSION=$(vips --version | head -n1)
        print_success "libvips found: $VIPS_VERSION"
    else
        print_info "libvips not found locally (not required for Docker deployment)"
    fi
}

# Generate secure random hex string
generate_secure_key() {
    if command -v openssl &> /dev/null; then
        openssl rand -hex 64
    else
        # Fallback to /dev/urandom
        head -c 64 /dev/urandom | od -An -tx1 | tr -d ' \n'
    fi
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
        read -p "$(echo -e ${CYAN}Choose cache type [1-4]:${NC} )" cache_choice
        case $cache_choice in
            1)
                CACHE_TYPE="memory"
                CACHE_MEMORY_CAPACITY="1000"
                print_success "Memory cache selected (1000 entries)"
                break
                ;;
            2)
                CACHE_TYPE="disk"
                CACHE_DISK_PATH="/var/imgforge/cache"
                CACHE_DISK_CAPACITY="10737418240"  # 10 GB
                print_success "Disk cache selected (10 GB at $CACHE_DISK_PATH)"
                break
                ;;
            3)
                CACHE_TYPE="hybrid"
                CACHE_MEMORY_CAPACITY="1000"
                CACHE_DISK_PATH="/var/imgforge/cache"
                CACHE_DISK_CAPACITY="10737418240"  # 10 GB
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
        read -p "$(echo -e ${CYAN}Enable Prometheus + Grafana monitoring? [y/N]:${NC} )" monitoring_choice
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

# Pull Docker images
pull_images() {
    print_info "Pulling imgforge Docker image..."
    
    if docker pull $IMGFORGE_IMAGE; then
        print_success "imgforge image pulled successfully"
    else
        print_error "Failed to pull imgforge image"
        print_info "Check your internet connection and Docker registry access"
        exit 1
    fi
    
    if [ "$ENABLE_MONITORING" = true ]; then
        print_info "Pulling monitoring images..."
        
        if docker pull prom/prometheus:latest && docker pull grafana/grafana:latest; then
            print_success "Monitoring images pulled successfully"
        else
            print_error "Failed to pull monitoring images"
            exit 1
        fi
    fi
}

# Create deployment directory structure
create_deployment_structure() {
    print_info "Creating deployment directory structure..."
    
    mkdir -p "$DEPLOYMENT_DIR"
    
    if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
        sudo mkdir -p "$CACHE_DISK_PATH"
        sudo chown -R "$USER:$USER" "$CACHE_DISK_PATH" 2>/dev/null || true
    fi
    
    if [ "$ENABLE_MONITORING" = true ]; then
        mkdir -p "$DEPLOYMENT_DIR/prometheus"
        mkdir -p "$DEPLOYMENT_DIR/grafana-dashboards"
    fi
    
    print_success "Directory structure created"
}

# Generate configuration files
generate_configs() {
    print_info "Generating configuration files..."
    
    # Generate keys if not provided
    if [ -z "$IMGFORGE_KEY" ]; then
        IMGFORGE_KEY=$(generate_secure_key)
    fi
    if [ -z "$IMGFORGE_SALT" ]; then
        IMGFORGE_SALT=$(generate_secure_key)
    fi
    
    # Create .env file
    cat > "$DEPLOYMENT_DIR/.env" << EOF
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
        echo "IMGFORGE_CACHE_TYPE=$CACHE_TYPE" >> "$DEPLOYMENT_DIR/.env"
        
        if [ "$CACHE_TYPE" = "memory" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
            echo "IMGFORGE_CACHE_MEMORY_CAPACITY=$CACHE_MEMORY_CAPACITY" >> "$DEPLOYMENT_DIR/.env"
        fi
        
        if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
            echo "IMGFORGE_CACHE_DISK_PATH=$CACHE_DISK_PATH" >> "$DEPLOYMENT_DIR/.env"
            echo "IMGFORGE_CACHE_DISK_CAPACITY=$CACHE_DISK_CAPACITY" >> "$DEPLOYMENT_DIR/.env"
        fi
    fi
    
    if [ "$ENABLE_MONITORING" = true ]; then
        echo "" >> "$DEPLOYMENT_DIR/.env"
        echo "# Monitoring Configuration" >> "$DEPLOYMENT_DIR/.env"
        echo "IMGFORGE_PROMETHEUS_BIND=9000" >> "$DEPLOYMENT_DIR/.env"
    fi
    
    # Create docker-compose.yml
    cat > "$DEPLOYMENT_DIR/docker-compose.yml" << 'EOF'
name: imgforge-deployment

services:
  imgforge:
    image: ghcr.io/imgforger/imgforge:latest
    container_name: imgforge
    restart: unless-stopped
    ports:
      - "3000:3000"
EOF

    if [ "$ENABLE_MONITORING" = true ]; then
        cat >> "$DEPLOYMENT_DIR/docker-compose.yml" << 'EOF'
      - "9000:9000"
EOF
    fi

    cat >> "$DEPLOYMENT_DIR/docker-compose.yml" << 'EOF'
    env_file:
      - .env
EOF

    if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
        cat >> "$DEPLOYMENT_DIR/docker-compose.yml" << EOF
    volumes:
      - $CACHE_DISK_PATH:/var/imgforge/cache
EOF
    fi

    if [ "$ENABLE_MONITORING" = true ]; then
        cat >> "$DEPLOYMENT_DIR/docker-compose.yml" << 'EOF'
    networks:
      - imgforge-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/status"]
      interval: 30s
      timeout: 10s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    container_name: imgforge-prometheus
    restart: unless-stopped
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/usr/share/prometheus/console_libraries'
      - '--web.console.templates=/usr/share/prometheus/consoles'
    networks:
      - imgforge-network

  grafana:
    image: grafana/grafana:latest
    container_name: imgforge-grafana
    restart: unless-stopped
    ports:
      - "3001:3000"
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana-dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana-datasources.yml:/etc/grafana/provisioning/datasources/datasources.yml
    networks:
      - imgforge-network
    depends_on:
      - prometheus

networks:
  imgforge-network:
    driver: bridge

volumes:
  prometheus-data:
  grafana-data:
EOF
    fi

    # Create prometheus.yml if monitoring is enabled
    if [ "$ENABLE_MONITORING" = true ]; then
        cat > "$DEPLOYMENT_DIR/prometheus/prometheus.yml" << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'imgforge'
    static_configs:
      - targets: ['imgforge:9000']
    metrics_path: '/metrics'
    scrape_interval: 10s
EOF

        # Create grafana-datasources.yml
        cat > "$DEPLOYMENT_DIR/grafana-datasources.yml" << 'EOF'
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    uid: prometheus
    url: http://prometheus:9090
    isDefault: true
    editable: true
EOF

        # Create dashboard provisioning config
        mkdir -p "$DEPLOYMENT_DIR/grafana-dashboards"
        cat > "$DEPLOYMENT_DIR/grafana-dashboards/dashboard-provisioning.yml" << 'EOF'
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
      path: /etc/grafana/provisioning/dashboards
EOF
    fi
    
    print_success "Configuration files generated"
}

# Start services
start_services() {
    print_info "Starting imgforge services..."
    
    cd "$DEPLOYMENT_DIR"
    
    if docker compose up -d; then
        print_success "Services started successfully"
    else
        print_error "Failed to start services"
        print_info "Check logs with: cd $DEPLOYMENT_DIR && docker compose logs"
        exit 1
    fi
    
    # Wait for services to be ready
    print_info "Waiting for services to be ready..."
    sleep 5
    
    # Check if imgforge is responding
    local retries=0
    local max_retries=30
    while [ $retries -lt $max_retries ]; do
        if curl -sf http://localhost:$IMGFORGE_PORT/status > /dev/null 2>&1; then
            print_success "imgforge is ready!"
            break
        fi
        retries=$((retries + 1))
        sleep 2
    done
    
    if [ $retries -eq $max_retries ]; then
        print_warning "imgforge may not be responding. Check logs with: docker logs imgforge"
    fi
}

# Print final information
print_final_info() {
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                                                            ║${NC}"
    echo -e "${GREEN}║        ${CYAN}🎉  imgforge Deployment Successful!  🎉${NC}  ${GREEN}║${NC}"
    echo -e "${GREEN}║                                                            ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${CYAN}Service Information:${NC}"
    echo -e "  ${GREEN}•${NC} imgforge:      http://localhost:$IMGFORGE_PORT"
    echo -e "  ${GREEN}•${NC} Health check:  http://localhost:$IMGFORGE_PORT/status"
    echo -e "  ${GREEN}•${NC} System info:   http://localhost:$IMGFORGE_PORT/info"
    
    if [ "$ENABLE_MONITORING" = true ]; then
        echo ""
        echo -e "${CYAN}Monitoring Services:${NC}"
        echo -e "  ${GREEN}•${NC} Prometheus:    http://localhost:$PROMETHEUS_PORT"
        echo -e "  ${GREEN}•${NC} Grafana:       http://localhost:$GRAFANA_PORT"
        echo -e "                    ${YELLOW}Username:${NC} admin"
        echo -e "                    ${YELLOW}Password:${NC} admin"
        echo -e "  ${GREEN}•${NC} Metrics:       http://localhost:$METRICS_PORT/metrics"
    fi
    
    echo ""
    echo -e "${CYAN}Deployment Location:${NC}"
    echo -e "  ${GREEN}•${NC} Config files:  $DEPLOYMENT_DIR"
    echo -e "  ${GREEN}•${NC} Environment:   $DEPLOYMENT_DIR/.env"
    
    if [ "$CACHE_TYPE" = "disk" ] || [ "$CACHE_TYPE" = "hybrid" ]; then
        echo -e "  ${GREEN}•${NC} Cache path:    $CACHE_DISK_PATH"
    fi
    
    echo ""
    echo -e "${CYAN}Useful Commands:${NC}"
    echo -e "  ${GREEN}•${NC} View logs:         ${YELLOW}docker logs imgforge -f${NC}"
    echo -e "  ${GREEN}•${NC} Restart service:   ${YELLOW}cd $DEPLOYMENT_DIR && docker compose restart${NC}"
    echo -e "  ${GREEN}•${NC} Stop service:      ${YELLOW}cd $DEPLOYMENT_DIR && docker compose down${NC}"
    echo -e "  ${GREEN}•${NC} Update image:      ${YELLOW}cd $DEPLOYMENT_DIR && docker compose pull && docker compose up -d${NC}"
    
    echo ""
    echo -e "${CYAN}Security Notes:${NC}"
    echo -e "  ${YELLOW}⚠${NC}  Your security keys are stored in: ${YELLOW}$DEPLOYMENT_DIR/.env${NC}"
    echo -e "  ${YELLOW}⚠${NC}  Keep this file secure and do not share it publicly"
    echo -e "  ${YELLOW}⚠${NC}  URLs must be HMAC-signed for security (see documentation)"
    
    if [ "$ENABLE_MONITORING" = true ]; then
        echo ""
        echo -e "${YELLOW}⚠  Change the default Grafana password after first login!${NC}"
    fi
    
    echo ""
    echo -e "${CYAN}Next Steps:${NC}"
    echo -e "  ${GREEN}1.${NC} Read the documentation: ${BLUE}https://github.com/ImgForger/imgforge${NC}"
    echo -e "  ${GREEN}2.${NC} Test a health check:    ${YELLOW}curl http://localhost:$IMGFORGE_PORT/status${NC}"
    echo -e "  ${GREEN}3.${NC} Generate signed URLs using your IMGFORGE_KEY and IMGFORGE_SALT"
    
    echo ""
    echo -e "${GREEN}Thank you for using imgforge!${NC} 🚀"
    echo ""
}

# Main deployment flow
main() {
    print_header
    
    print_info "Starting imgforge deployment process..."
    echo ""
    
    # Pre-flight checks
    check_docker
    check_libvips
    
    # Get user preferences
    ask_cache_config
    ask_monitoring_config
    
    # Check ports
    check_required_ports
    
    # Pull images
    pull_images
    
    # Create structure
    create_deployment_structure
    
    # Generate configs
    generate_configs
    
    # Start services
    start_services
    
    # Show final information
    print_final_info
}

# Run main function
main
