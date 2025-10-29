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
DEPLOYMENT_DIR="$HOME/.imgforge"
INSTALL_DIR="/opt/imgforge"
CONFIG_DIR="/etc/imgforge"
CACHE_DIR="/var/lib/imgforge/cache"
LOG_DIR="/var/log/imgforge"
DATA_DIR="/var/lib/imgforge"

# Helper functions
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

print_header() {
    echo ""
    echo -e "${CYAN}╔═════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}║                  ${RED}!! imgforge Uninstaller (Systemd) !!${NC}               ${CYAN}║${NC}"
    echo -e "${CYAN}║                                                                     ║${NC}"
    echo -e "${CYAN}╚═════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
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
        input_value="n"
    fi

    printf -v "$result_var" '%s' "$input_value"
}

# Stop services
stop_services() {
    print_info "Stopping services..."

    if systemctl is-active --quiet imgforge.service 2>/dev/null; then
        sudo systemctl stop imgforge.service
        print_success "imgforge service stopped"
    fi

    if systemctl is-active --quiet prometheus.service 2>/dev/null; then
        sudo systemctl stop prometheus.service
        print_success "Prometheus service stopped"
    fi

    if systemctl is-active --quiet grafana-server.service 2>/dev/null; then
        sudo systemctl stop grafana-server.service
        print_success "Grafana service stopped"
    fi
}

# Disable services
disable_services() {
    print_info "Disabling services..."

    if systemctl is-enabled --quiet imgforge.service 2>/dev/null; then
        sudo systemctl disable imgforge.service
        print_success "imgforge service disabled"
    fi

    if systemctl is-enabled --quiet prometheus.service 2>/dev/null; then
        sudo systemctl disable prometheus.service
        print_success "Prometheus service disabled"
    fi

    if systemctl is-enabled --quiet grafana-server.service 2>/dev/null; then
        sudo systemctl disable grafana-server.service
        print_success "Grafana service disabled"
    fi
}

# Remove service files
remove_service_files() {
    print_info "Removing systemd service files..."

    if [ -f /etc/systemd/system/imgforge.service ]; then
        sudo rm /etc/systemd/system/imgforge.service
        print_success "Removed imgforge.service"
    fi

    if [ -f /etc/systemd/system/prometheus.service ]; then
        sudo rm /etc/systemd/system/prometheus.service
        print_success "Removed prometheus.service"
    fi

    sudo systemctl daemon-reload
}

# Remove binaries
remove_binaries() {
    print_info "Removing imgforge binary..."

    if [ -d "$INSTALL_DIR" ]; then
        sudo rm -rf "$INSTALL_DIR"
        print_success "Removed $INSTALL_DIR"
    fi

    if [ -f /usr/local/bin/prometheus ]; then
        sudo rm /usr/local/bin/prometheus
        print_success "Removed Prometheus binary"
    fi

    if [ -f /usr/local/bin/promtool ]; then
        sudo rm /usr/local/bin/promtool
        print_success "Removed Promtool binary"
    fi
}

# Remove configuration files
remove_configs() {
    print_info "Removing configuration files..."

    if [ -d "$CONFIG_DIR" ]; then
        sudo rm -rf "$CONFIG_DIR"
        print_success "Removed $CONFIG_DIR"
    fi

    if [ -d /etc/prometheus ]; then
        prompt_read "${YELLOW}Remove Prometheus configuration? [y/N]:${NC} " remove_prom_config
        if [[ "$remove_prom_config" =~ ^[Yy]$ ]]; then
            sudo rm -rf /etc/prometheus
            print_success "Removed Prometheus configuration"
        fi
    fi

    if [ -d /etc/grafana ]; then
        prompt_read "${YELLOW}Remove Grafana configuration? [y/N]:${NC} " remove_grafana_config
        if [[ "$remove_grafana_config" =~ ^[Yy]$ ]]; then
            sudo rm -rf /etc/grafana
            print_success "Removed Grafana configuration"
        fi
    fi
}

# Remove data directories
remove_data() {
    print_info "Removing data directories..."

    prompt_read "${YELLOW}Remove all data (including cache and logs)? [y/N]:${NC} " remove_data_choice

    if [[ "$remove_data_choice" =~ ^[Yy]$ ]]; then
        if [ -d "$DATA_DIR" ]; then
            sudo rm -rf "$DATA_DIR"
            print_success "Removed $DATA_DIR"
        fi

        if [ -d "$LOG_DIR" ]; then
            sudo rm -rf "$LOG_DIR"
            print_success "Removed $LOG_DIR"
        fi

        if [ -d "$DEPLOYMENT_DIR" ]; then
            rm -rf "$DEPLOYMENT_DIR"
            print_success "Removed $DEPLOYMENT_DIR"
        fi
    else
        print_info "Data directories preserved"
        print_warning "Configuration backup still available at: $DEPLOYMENT_DIR/imgforge.env.backup"
    fi
}

# Uninstall packages
uninstall_packages() {
    echo ""
    prompt_read "${YELLOW}Remove installed packages (Prometheus, Grafana, libvips)? [y/N]:${NC} " remove_packages

    if [[ "$remove_packages" =~ ^[Yy]$ ]]; then
        print_info "Uninstalling packages..."
        local packages_removed=false

        if command -v apt-get &> /dev/null; then
            local packages=()
            for pkg in grafana prometheus libvips libvips-dev libvips-tools; do
                if dpkg -s "$pkg" >/dev/null 2>&1; then
                    packages+=("$pkg")
                fi
            done

            if [ "${#packages[@]}" -gt 0 ]; then
                sudo apt-get remove -y "${packages[@]}"
                print_success "Removed packages: ${packages[*]}"
                packages_removed=true
            fi

            if [ -f /etc/apt/sources.list.d/grafana.list ]; then
                sudo rm -f /etc/apt/sources.list.d/grafana.list
                print_success "Removed Grafana APT repository"
            fi

            if [ -f /etc/apt/keyrings/grafana.gpg ]; then
                sudo rm -f /etc/apt/keyrings/grafana.gpg
                print_success "Removed Grafana APT key"
            fi
        elif command -v yum &> /dev/null; then
            local packages=()
            for pkg in grafana prometheus vips vips-devel; do
                if rpm -q "$pkg" >/dev/null 2>&1; then
                    packages+=("$pkg")
                fi
            done

            if [ "${#packages[@]}" -gt 0 ]; then
                sudo yum remove -y "${packages[@]}"
                print_success "Removed packages: ${packages[*]}"
                packages_removed=true
            fi

            if [ -f /etc/yum.repos.d/grafana.repo ]; then
                sudo rm -f /etc/yum.repos.d/grafana.repo
                print_success "Removed Grafana YUM repo"
            fi
        elif command -v dnf &> /dev/null; then
            local packages=()
            for pkg in grafana prometheus vips vips-devel; do
                if rpm -q "$pkg" >/dev/null 2>&1; then
                    packages+=("$pkg")
                fi
            done

            if [ "${#packages[@]}" -gt 0 ]; then
                sudo dnf remove -y "${packages[@]}"
                print_success "Removed packages: ${packages[*]}"
                packages_removed=true
            fi

            if [ -f /etc/yum.repos.d/grafana.repo ]; then
                sudo rm -f /etc/yum.repos.d/grafana.repo
                print_success "Removed Grafana DNF repo"
            fi
        elif command -v pacman &> /dev/null; then
            local packages=()
            for pkg in grafana prometheus libvips; do
                if pacman -Qi "$pkg" >/dev/null 2>&1; then
                    packages+=("$pkg")
                fi
            done

            if [ "${#packages[@]}" -gt 0 ]; then
                sudo pacman -Rns --noconfirm "${packages[@]}"
                print_success "Removed packages: ${packages[*]}"
                packages_removed=true
            fi
        fi

        if [ -d /opt/grafana ]; then
            sudo rm -rf /opt/grafana
            sudo rm -f /usr/local/bin/grafana-server
            print_success "Removed Grafana binary installation"
            packages_removed=true
        fi

        if [ "$packages_removed" = false ]; then
            print_info "No matching packages were removed"
        fi
    else
        print_info "Packages preserved"
    fi
}

# Display summary
display_summary() {
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                                                    ║${NC}"
    echo -e "${GREEN}║      ${CYAN}!! imgforge has been uninstalled !!${NC}           ${GREEN}║${NC}"
    echo -e "${GREEN}║                                                    ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════╝${NC}"

    if [ -f "$DEPLOYMENT_DIR/imgforge.env.backup" ]; then
        echo -e "${BLUE}Note:${NC} Configuration backup is still available at:"
        echo -e "  ${YELLOW}$DEPLOYMENT_DIR/imgforge.env.backup${NC}"
        echo ""
        echo "Keep this file if you plan to reinstall imgforge later."
        echo "Otherwise, you can remove it with:"
        echo "  rm -rf $DEPLOYMENT_DIR"
        echo ""
    fi

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Main execution
main() {
    print_header

    print_warning "This will uninstall imgforge and optionally remove all data."
    echo ""
    prompt_read "${YELLOW}Are you sure you want to continue? [y/N]:${NC} " confirm

    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        print_info "Uninstallation cancelled"
        exit 0
    fi

    echo ""

    stop_services
    disable_services
    remove_service_files
    remove_binaries
    remove_configs
    remove_data
    uninstall_packages

    display_summary
}

main "$@"
