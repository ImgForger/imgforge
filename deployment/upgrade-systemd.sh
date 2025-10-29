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
INSTALL_DIR="/opt/imgforge"

# Helper functions
print_header() {
    echo ""
    echo -e "${CYAN}╔═════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}║                  ${BLUE}imgforge Upgrade Script${NC}                          ${CYAN}║${NC}"
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

# Check if imgforge is installed
check_installation() {
    if [ ! -f "$INSTALL_DIR/imgforge" ]; then
        print_error "imgforge is not installed at $INSTALL_DIR/imgforge"
        print_info "Please run the deployment script first"
        exit 1
    fi
    
    if ! systemctl list-unit-files | grep -q "imgforge.service"; then
        print_error "imgforge service is not installed"
        print_info "Please run the deployment script first"
        exit 1
    fi
    
    print_success "imgforge installation found"
}

# Get current installed version
get_current_version() {
    if [ -f "$INSTALL_DIR/imgforge" ]; then
        local version_output=$("$INSTALL_DIR/imgforge" --version 2>&1 || echo "unknown")
        if [[ "$version_output" =~ ([0-9]+\.[0-9]+\.[0-9]+) ]]; then
            echo "${BASH_REMATCH[1]}"
        else
            echo "unknown"
        fi
    else
        echo "not installed"
    fi
}

# Get latest release version from GitHub
get_latest_release() {
    local latest_release
    
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
        print_info "Check your internet connection and try again"
        exit 1
    fi
    
    echo "$latest_release"
}

# Check if update is needed
check_update_needed() {
    local current_version=$1
    local latest_version=$2
    
    print_info "Current version: $current_version"
    print_info "Latest version:  $latest_version"
    
    if [ "$current_version" = "$latest_version" ]; then
        return 1
    fi
    
    return 0
}

# Download and install imgforge binary
download_imgforge() {
    local version=$1
    
    print_info "Downloading imgforge $version..."
    
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
    
    local download_url="https://github.com/ImgForger/imgforge/releases/download/${version}/imgforge-linux-${binary_arch}.tar.gz"
    
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
    
    print_success "Binary downloaded and extracted"
    
    echo "$tmp_dir"
}

# Backup current binary
backup_current_binary() {
    local backup_file="$INSTALL_DIR/imgforge.backup.$(date +%Y%m%d_%H%M%S)"
    
    print_info "Creating backup of current binary..."
    
    if sudo cp "$INSTALL_DIR/imgforge" "$backup_file"; then
        print_success "Backup created: $backup_file"
        echo "$backup_file"
    else
        print_error "Failed to create backup"
        exit 1
    fi
}

# Install new binary
install_binary() {
    local tmp_dir=$1
    
    print_info "Installing new binary..."
    
    if sudo cp "$tmp_dir/imgforge" "$INSTALL_DIR/imgforge"; then
        sudo chmod +x "$INSTALL_DIR/imgforge"
        print_success "New binary installed"
    else
        print_error "Failed to install new binary"
        exit 1
    fi
}

# Rollback to backup
rollback_binary() {
    local backup_file=$1
    
    print_warning "Rolling back to previous version..."
    
    if sudo cp "$backup_file" "$INSTALL_DIR/imgforge"; then
        sudo chmod +x "$INSTALL_DIR/imgforge"
        print_success "Rolled back to previous version"
    else
        print_error "Failed to rollback"
    fi
}

# Stop service
stop_service() {
    print_info "Stopping imgforge service..."
    
    if sudo systemctl stop imgforge.service; then
        print_success "Service stopped"
    else
        print_error "Failed to stop service"
        exit 1
    fi
}

# Start service
start_service() {
    print_info "Starting imgforge service..."
    
    if sudo systemctl start imgforge.service; then
        sleep 2
        print_success "Service started"
    else
        print_error "Failed to start service"
        return 1
    fi
    
    return 0
}

# Verify service is running
verify_service() {
    print_info "Verifying service status..."
    
    if sudo systemctl is-active --quiet imgforge.service; then
        print_success "Service is running"
        return 0
    else
        print_error "Service is not running"
        return 1
    fi
}

# Health check
health_check() {
    print_info "Performing health check..."
    
    local max_attempts=5
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if curl -sf http://localhost:3000/status > /dev/null 2>&1; then
            print_success "Health check passed"
            return 0
        fi
        
        print_info "Waiting for service to be ready (attempt $attempt/$max_attempts)..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    print_error "Health check failed after $max_attempts attempts"
    return 1
}

# Cleanup
cleanup() {
    local tmp_dir=$1
    
    if [ -n "$tmp_dir" ] && [ -d "$tmp_dir" ]; then
        rm -rf "$tmp_dir"
    fi
}

# Display summary
display_summary() {
    local old_version=$1
    local new_version=$2
    
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}              Upgrade Summary${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "${GREEN}✓ imgforge has been successfully upgraded!${NC}"
    echo ""
    echo -e "${BLUE}Version Information:${NC}"
    echo -e "  • Previous version: $old_version"
    echo -e "  • Current version:  $new_version"
    echo ""
    echo -e "${BLUE}Service Status:${NC}"
    echo -e "  • Status: $(sudo systemctl is-active imgforge.service)"
    echo -e "  • Check logs: sudo journalctl -u imgforge -n 50"
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Main execution
main() {
    print_header
    
    check_root
    check_installation
    
    print_info "Checking for updates..."
    
    local current_version=$(get_current_version)
    local latest_version=$(get_latest_release)
    
    if ! check_update_needed "$current_version" "$latest_version"; then
        print_success "imgforge is already up to date ($current_version)"
        exit 0
    fi
    
    echo ""
    print_warning "A new version of imgforge is available!"
    echo ""
    read -p "$(echo -e "${CYAN}Do you want to upgrade from $current_version to $latest_version? [y/N]:${NC} ")" -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Upgrade cancelled"
        exit 0
    fi
    
    echo ""
    
    local backup_file=$(backup_current_binary)
    
    stop_service
    
    local tmp_dir=$(download_imgforge "$latest_version")
    
    install_binary "$tmp_dir"
    
    if ! start_service; then
        print_error "Service failed to start, rolling back..."
        rollback_binary "$backup_file"
        start_service
        cleanup "$tmp_dir"
        exit 1
    fi
    
    if ! verify_service; then
        print_error "Service verification failed, rolling back..."
        stop_service
        rollback_binary "$backup_file"
        start_service
        cleanup "$tmp_dir"
        exit 1
    fi
    
    if ! health_check; then
        print_warning "Health check failed, but service is running"
        print_info "You may want to check the logs: sudo journalctl -u imgforge -n 50"
        read -p "$(echo -e "${CYAN}Do you want to rollback? [y/N]:${NC} ")" -n 1 -r
        echo
        
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Rolling back..."
            stop_service
            rollback_binary "$backup_file"
            start_service
            cleanup "$tmp_dir"
            exit 1
        fi
    fi
    
    cleanup "$tmp_dir"
    
    print_info "Cleaning up old backups (keeping last 3)..."
    sudo ls -t "$INSTALL_DIR"/imgforge.backup.* 2>/dev/null | tail -n +4 | xargs -r sudo rm -f
    
    local new_version=$(get_current_version)
    display_summary "$current_version" "$new_version"
}

main "$@"
