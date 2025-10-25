# ImgForge Monitoring with Prometheus and Grafana

This document describes the monitoring setup for ImgForge using Prometheus and Grafana.

## Overview

The monitoring stack includes:
- **Prometheus**: Metrics collection and storage
- **Grafana**: Metrics visualization and dashboarding
- **ImgForge**: Application with built-in `/metrics` endpoint

## Quick Start

### Starting the Full Stack

```bash
docker-compose up -d
```

This will start three services:
- `imgforge` on port 3000
- `prometheus` on port 9090
- `grafana` on port 3001

### Accessing the Services

- **ImgForge**: http://localhost:3000
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3001
  - Username: `admin`
  - Password: `admin`

## Grafana Dashboard

The ImgForge dashboard is automatically provisioned and includes the following panels:

### Overview Panels
- **HTTP Status Codes**: Rate of responses by status code
- **Total HTTP Requests**: Total number of HTTP requests processed

### Performance Metrics
- **HTTP Request Duration**: p50, p95, and p99 latencies by endpoint
- **Image Processing Duration**: Processing time percentiles by output format

### Cache Metrics
- **Cache Hit Rate**: Hit rate percentage by cache type
- **Cache Hits vs Misses**: Rate of cache hits and misses

### Image Processing
- **Processed Images by Format**: Rate of processed images grouped by output format
- **Total Processed Images**: Counter of all processed images

### Memory Tracking
- **libvips Memory Usage**: Current and peak memory usage
- **libvips Active Allocations**: Number of active memory allocations

### Source Fetching
- **Source Image Fetch Duration**: p50, p95, and p99 latencies for fetching source images
- **Source Images Fetched by Status**: Rate of fetches by success/error status

## Available Metrics

ImgForge exposes the following Prometheus metrics at `/metrics`:

| Metric                                | Type      | Labels           | Description                  |
|---------------------------------------|-----------|------------------|------------------------------|
| `http_requests_duration_seconds`      | Histogram | `method`, `path` | HTTP request duration        |
| `image_processing_duration_seconds`   | Histogram | `format`         | Image processing duration    |
| `source_image_fetch_duration_seconds` | Histogram | -                | Source image fetch duration  |
| `processed_images_total`              | Counter   | `format`         | Total processed images       |
| `source_images_fetched_total`         | Counter   | `status`         | Total source images fetched  |
| `cache_hits_total`                    | Counter   | `cache_type`     | Total cache hits             |
| `cache_misses_total`                  | Counter   | `cache_type`     | Total cache misses           |
| `status_codes_total`                  | Counter   | `status`         | Total HTTP response codes    |
| `vips_tracked_mem_bytes`              | Gauge     | -                | Current libvips memory usage |
| `vips_tracked_mem_highwater_bytes`    | Gauge     | -                | Peak libvips memory usage    |
| `vips_tracked_allocs`                 | Gauge     | -                | Active libvips allocations   |

## Configuration

### Prometheus Configuration

The Prometheus configuration is in `prometheus.yml`:
- Scrape interval: 15 seconds
- ImgForge scrape interval: 10 seconds
- Target: `imgforge:9000/metrics`

### Grafana Configuration

Grafana is configured via:
- `grafana-datasources.yml`: Configures Prometheus as the default datasource
- `grafana-dashboards/dashboard-provisioning.yml`: Enables dashboard auto-provisioning
- `grafana-dashboards/imgforge-dashboard.json`: The main ImgForge dashboard

## Customization

### Modifying the Dashboard

You can customize the dashboard in two ways:

1. **Through the Grafana UI**: 
   - Log into Grafana
   - Navigate to the "ImgForge Monitoring Dashboard"
   - Click the settings icon and select "Edit"
   - Make your changes and save

2. **Editing the JSON file**:
   - Edit `grafana-dashboards/imgforge-dashboard.json`
   - Restart the Grafana container: `docker-compose restart grafana`

### Adding More Metrics

To add new metrics to ImgForge:
1. Define the metric in `src/monitoring.rs`
2. Register it in the `register_metrics()` function
3. Update the Grafana dashboard JSON to visualize the new metric

## Troubleshooting

### Metrics Not Appearing

1. Check that ImgForge is running: `curl http://localhost:9000/metrics`
2. Verify Prometheus can reach ImgForge: Check Prometheus targets at http://localhost:9090/targets
3. Check Prometheus logs: `docker-compose logs prometheus`

### Dashboard Not Loading

1. Verify the datasource is configured: Go to Configuration > Data Sources in Grafana
2. Check Grafana logs: `docker-compose logs grafana`
3. Manually import the dashboard: Copy the content of `imgforge-dashboard.json` and import via Grafana UI

### Data Not Persisting

The Docker Compose setup includes persistent volumes for both Prometheus and Grafana:
- `prometheus-data`: Stores Prometheus time-series data
- `grafana-data`: Stores Grafana configuration and dashboards

To reset all data:
```bash
docker-compose down -v
```

## Production Considerations

For production deployments, consider:

1. **Security**:
   - Change the default Grafana admin password
   - Add authentication to Prometheus
   - Use HTTPS for all services

2. **Persistence**:
   - Use external volumes or cloud storage for data persistence
   - Configure retention policies for Prometheus

3. **Scalability**:
   - Consider using Thanos or Cortex for long-term Prometheus storage
   - Set up alerting rules in Prometheus
   - Configure alert notifications in Grafana

4. **Networking**:
   - Restrict access to monitoring services
   - Use a reverse proxy for external access
