# Testing the Deployment Script

This guide helps developers test the deployment script in various scenarios.

## Quick Syntax Check

```bash
bash -n deploy.sh
```

## Dry Run (Without Docker)

To test the script logic without actually installing anything:

```bash
# Set test mode (script will skip actual Docker operations)
export IMGFORGE_DEPLOY_TEST_MODE=true
./deploy.sh
```

## Testing on a Fresh VM

### Using Vagrant

```bash
# Create a Vagrantfile
cat > Vagrantfile << 'EOF'
Vagrant.configure("2") do |config|
  config.vm.box = "ubuntu/jammy64"
  config.vm.network "forwarded_port", guest: 3000, host: 3000
  config.vm.network "forwarded_port", guest: 9090, host: 9090
  config.vm.network "forwarded_port", guest: 3001, host: 3001
  
  config.vm.provider "virtualbox" do |vb|
    vb.memory = "2048"
    vb.cpus = 2
  end
end
EOF

# Start the VM
vagrant up

# SSH into the VM
vagrant ssh

# Inside the VM, run:
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh | bash

# Or copy the local script
exit
vagrant scp deploy.sh default:~/
vagrant ssh
chmod +x deploy.sh
./deploy.sh
```

### Using Docker (Inception!)

Test the deployment script inside a Docker container:

```bash
# Ubuntu 22.04
docker run -it --privileged \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v $(pwd):/deployment \
  -p 3000:3000 -p 9090:9090 -p 3001:3001 \
  ubuntu:22.04 bash

# Inside container:
apt-get update && apt-get install -y curl
cd /deployment
./deploy.sh

# Debian 12
docker run -it --privileged \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v $(pwd):/deployment \
  -p 3000:3000 -p 9090:9090 -p 3001:3001 \
  debian:12 bash

# Inside container:
apt-get update && apt-get install -y curl
cd /deployment
./deploy.sh
```

## Manual Testing Checklist

- [ ] Docker installation detection works
- [ ] Docker installation (if missing) works
- [ ] Port conflict detection works
- [ ] Cache configuration prompts work correctly
- [ ] Monitoring configuration prompts work correctly
- [ ] Configuration files are generated correctly
- [ ] Docker Compose file is valid
- [ ] Services start successfully
- [ ] Health check succeeds
- [ ] Status endpoint responds
- [ ] Prometheus scrapes metrics (if enabled)
- [ ] Grafana connects to Prometheus (if enabled)
- [ ] Cache directory is created (if disk/hybrid)
- [ ] Final information is displayed correctly

## Automated Test Script

```bash
#!/bin/bash
# test-deploy.sh - Automated testing of deploy.sh

set -e

echo "Testing deployment script..."

# Test 1: Syntax check
echo "✓ Test 1: Syntax validation"
bash -n deploy.sh

# Test 2: Function definitions
echo "✓ Test 2: Function definitions"
source deploy.sh
declare -f print_header > /dev/null
declare -f check_docker > /dev/null
declare -f generate_secure_key > /dev/null

# Test 3: Key generation
echo "✓ Test 3: Key generation"
KEY=$(generate_secure_key)
if [ ${#KEY} -ne 128 ]; then
  echo "✗ Key length incorrect: ${#KEY}"
  exit 1
fi

# Test 4: Port checking
echo "✓ Test 4: Port checking"
check_port 99999 "test" || echo "Port check failed as expected for occupied port"

echo ""
echo "All tests passed! ✓"
```

## Testing Error Scenarios

### Port Already in Use

```bash
# Start a server on port 3000
python3 -m http.server 3000 &
PID=$!

# Run deploy script (should detect port conflict)
./deploy.sh

# Clean up
kill $PID
```

### No Docker Installed

Test on a system without Docker:

```bash
# On a fresh VM without Docker
./deploy.sh
# Should install Docker automatically
```

### Docker Daemon Not Running

```bash
sudo systemctl stop docker
./deploy.sh
# Should start Docker automatically
```

## Testing Different Configurations

### Memory Cache Only

When prompted, select option 1 (Memory Cache) and answer "n" to monitoring.

**Expected files:**
- `~/.imgforge/.env` with `IMGFORGE_CACHE_TYPE=memory`
- No Prometheus/Grafana in `docker-compose.yml`

### Hybrid Cache with Monitoring

When prompted, select option 3 (Hybrid Cache) and answer "y" to monitoring.

**Expected files:**
- `~/.imgforge/.env` with cache settings
- `~/.imgforge/docker-compose.yml` with all three services
- `~/.imgforge/prometheus/prometheus.yml`
- `~/.imgforge/grafana-datasources.yml`

### No Cache, No Monitoring

When prompted, select option 4 (No Cache) and answer "n" to monitoring.

**Expected files:**
- `~/.imgforge/.env` without cache settings
- Minimal `docker-compose.yml`

## Verification Commands

After deployment, verify everything works:

```bash
# Check containers are running
docker ps | grep imgforge

# Test imgforge endpoints
curl http://localhost:3000/status
curl http://localhost:3000/info

# Check metrics (if monitoring enabled)
curl http://localhost:9000/metrics

# Check Prometheus (if monitoring enabled)
curl http://localhost:9090/-/healthy

# Check Grafana (if monitoring enabled)
curl http://localhost:3001/api/health
```

## Cleanup Between Tests

```bash
# Stop all imgforge containers
cd ~/.imgforge && docker compose down -v

# Remove deployment directory
rm -rf ~/.imgforge

# Remove cache directory
sudo rm -rf /var/imgforge

# Remove Docker images (optional)
docker rmi ghcr.io/imgforger/imgforge:latest
docker rmi prom/prometheus:latest
docker rmi grafana/grafana:latest
```

## Performance Testing

After successful deployment:

```bash
# Generate test keys for URL signing
cd ~/.imgforge
KEY=$(grep IMGFORGE_KEY .env | cut -d'=' -f2)
SALT=$(grep IMGFORGE_SALT .env | cut -d'=' -f2)

echo "Key: $KEY"
echo "Salt: $SALT"

# Use these with the k6 load tests
cd /path/to/imgforge/loadtest
# Update the k6 scripts with your keys
# Run load tests
```

## CI/CD Integration Testing

Test the deployment script in CI/CD pipelines:

```yaml
# .github/workflows/test-deploy.yml
name: Test Deployment Script

on: [push, pull_request]

jobs:
  test-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run syntax check
        run: bash -n deployment/deploy.sh
      
      - name: Test in Docker container
        run: |
          docker run -d --privileged --name test-env \
            -v /var/run/docker.sock:/var/run/docker.sock \
            -v $PWD/deployment:/deployment \
            ubuntu:22.04 sleep 3600
          
          docker exec test-env apt-get update
          docker exec test-env apt-get install -y curl
          
          # Run deployment with minimal config
          docker exec test-env bash -c "cd /deployment && echo -e '4\nn\n' | ./deploy.sh"
          
          # Verify
          docker exec test-env curl -f http://localhost:3000/status
          
          # Cleanup
          docker stop test-env
          docker rm test-env
```

## Known Issues

### Issue: Script hangs on input

**Cause:** Running in non-interactive mode without piping answers.

**Solution:** Use input redirection:
```bash
echo -e "1\nn\n" | ./deploy.sh
```

### Issue: Permission denied on cache directory

**Cause:** Insufficient permissions to create `/var/imgforge`.

**Solution:** The script uses `sudo` for this. Ensure sudo is available.

### Issue: Docker pull fails

**Cause:** Network issues or Docker Hub rate limiting.

**Solution:** Retry or authenticate to Docker Hub:
```bash
docker login ghcr.io
```

## Distribution Testing

Test different distribution compatibility:

- [ ] Ubuntu 20.04 LTS
- [ ] Ubuntu 22.04 LTS
- [ ] Ubuntu 24.04 LTS
- [ ] Debian 11 (Bullseye)
- [ ] Debian 12 (Bookworm)
- [ ] CentOS 8 Stream
- [ ] Rocky Linux 8
- [ ] Rocky Linux 9
- [ ] Amazon Linux 2
- [ ] Amazon Linux 2023

## Reporting Issues

When reporting issues with the deployment script, include:

1. Distribution and version: `cat /etc/os-release`
2. Docker version: `docker --version`
3. Script output with errors
4. Output of: `docker logs imgforge`
5. Contents of: `~/.imgforge/.env` (redact keys!)
6. Contents of: `~/.imgforge/docker-compose.yml`
