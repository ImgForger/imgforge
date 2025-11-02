#!/bin/bash

# imgforge Uninstall Script
# This script removes imgforge and all associated data

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Constants
DEPLOYMENT_DIR="$HOME/.imgforge"
CACHE_DIR="/var/imgforge"

print_header() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                                                    ║${NC}"
    echo -e "${CYAN}║         ${RED}!! imgforge Uninstall Script !!${NC}            ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                    ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

prompt_read() {
    local prompt_text="$1"
    local result_var="$2"
    local input_value

    if [ -t 0 ]; then
        read -rp "$(echo -e "$prompt_text")" input_value
    elif [ -r /dev/tty ]; then
        # shellcheck disable=SC2162
        read -rp "$(echo -e "$prompt_text")" input_value </dev/tty
    else
        print_error "No interactive terminal available for prompt: $prompt_text"
        exit 1
    fi

    printf -v "$result_var" '%s' "$input_value"
}

confirm_uninstall() {
    echo -e "${YELLOW}WARNING: This will remove imgforge and all associated data!${NC}"
    echo ""
    echo "The following will be removed:"
    echo "  • imgforge Docker containers"
    echo "  • Configuration files in $DEPLOYMENT_DIR"
    echo "  • Cache data in $CACHE_DIR (if exists)"
    echo "  • Docker volumes (prometheus-data, grafana-data)"
    echo ""
    prompt_read "${RED}Are you sure you want to continue? [y/N]:${NC} " confirm
    
    case $confirm in
        [Yy]*)
            return 0
            ;;
        *)
            print_info "Uninstall cancelled."
            exit 0
            ;;
    esac
}

stop_containers() {
    print_info "Stopping imgforge containers..."
    
    if [ -d "$DEPLOYMENT_DIR" ] && [ -f "$DEPLOYMENT_DIR/docker-compose.yml" ]; then
        cd "$DEPLOYMENT_DIR"
        if docker compose down -v 2>/dev/null; then
            print_success "Containers stopped and removed"
        else
            print_warning "Failed to stop containers via docker-compose"
            # Try to stop containers directly
            for container in imgforge imgforge-prometheus imgforge-grafana; do
                if docker ps -a --format '{{.Names}}' | grep -q "^${container}$"; then
                    docker stop "$container" 2>/dev/null || true
                    docker rm "$container" 2>/dev/null || true
                fi
            done
            print_success "Containers removed manually"
        fi
    else
        print_info "No docker-compose.yml found, checking for running containers..."
        # Try to stop containers directly
        local found=false
        for container in imgforge imgforge-prometheus imgforge-grafana; do
            if docker ps -a --format '{{.Names}}' | grep -q "^${container}$"; then
                docker stop "$container" 2>/dev/null || true
                docker rm "$container" 2>/dev/null || true
                found=true
            fi
        done
        if [ "$found" = true ]; then
            print_success "Containers removed"
        else
            print_info "No imgforge containers found"
        fi
    fi
}

remove_volumes() {
    print_info "Removing Docker volumes..."
    
    local removed=false
    for volume in imgforge-deployment_prometheus-data imgforge-deployment_grafana-data prometheus-data grafana-data imgforger_prometheus-data imgforger_grafana-data; do
        if docker volume ls --format '{{.Name}}' | grep -q "^${volume}$"; then
            docker volume rm "$volume" 2>/dev/null || true
            removed=true
        fi
    done
    
    if [ "$removed" = true ]; then
        print_success "Docker volumes removed"
    else
        print_info "No Docker volumes found"
    fi
}

remove_config() {
    print_info "Removing configuration files..."
    
    if [ -d "$DEPLOYMENT_DIR" ]; then
        rm -rf "$DEPLOYMENT_DIR"
        print_success "Configuration directory removed: $DEPLOYMENT_DIR"
    else
        print_info "No configuration directory found"
    fi
}

remove_cache() {
    print_info "Removing cache directory..."
    
    if [ -d "$CACHE_DIR" ]; then
        if [ -w "$CACHE_DIR" ]; then
            rm -rf "$CACHE_DIR"
            print_success "Cache directory removed: $CACHE_DIR"
        else
            sudo rm -rf "$CACHE_DIR"
            print_success "Cache directory removed: $CACHE_DIR (with sudo)"
        fi
    else
        print_info "No cache directory found"
    fi
}

remove_images() {
    echo ""
    prompt_read "${CYAN}Do you want to remove Docker images as well? [y/N]:${NC} " remove_imgs
    
    case $remove_imgs in
        [Yy]*)
            print_info "Removing Docker images..."
            local removed=false
            
            for image in ghcr.io/imgforger/imgforge:latest prom/prometheus:latest grafana/grafana:latest; do
                if docker images --format '{{.Repository}}:{{.Tag}}' | grep -q "^${image}$"; then
                    docker rmi "$image" 2>/dev/null || true
                    removed=true
                fi
            done
            
            if [ "$removed" = true ]; then
                print_success "Docker images removed"
            else
                print_info "No images found to remove"
            fi
            ;;
        *)
            print_info "Keeping Docker images"
            ;;
    esac
}

print_final() {
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                                                    ║${NC}"
    echo -e "${GREEN}║      ${CYAN}!! imgforge has been uninstalled !!${NC}           ${GREEN}║${NC}"
    echo -e "${GREEN}║                                                    ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════╝${NC}"
    echo ""
    print_info "If you installed Docker solely for imgforge, you can remove it with:"
    echo -e "       ${YELLOW}sudo apt-get remove docker-ce docker-ce-cli containerd.io${NC} (Ubuntu/Debian)"
    echo -e "       ${YELLOW}sudo yum remove docker-ce docker-ce-cli containerd.io${NC} (CentOS/RHEL)"
    echo ""
    print_info "Thank you for using imgforge!"
    echo ""
}

# Main uninstall flow
main() {
    print_header
    
    confirm_uninstall
    
    echo ""
    print_info "Starting uninstall process..."
    echo ""
    
    stop_containers
    remove_volumes
    remove_config
    remove_cache
    remove_images
    
    print_final
}

# Run main function
main
