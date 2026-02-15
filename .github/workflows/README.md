# GitHub Actions Workflows

## Docker Publish

Automatically builds and publishes Docker images to Docker Hub.

### Setup

1. **Create Docker Hub Access Token**
   - Go to https://hub.docker.com/settings/security
   - Click "New Access Token"
   - Name: `github-actions`
   - Permissions: Read, Write, Delete
   - Copy the token

2. **Add GitHub Secrets**
   - Go to your repo: Settings → Secrets and variables → Actions
   - Add two secrets:
     - `DOCKERHUB_USERNAME`: Your Docker Hub username
     - `DOCKERHUB_TOKEN`: The access token from step 1

### Triggers

The workflow runs on:
- **Push to `dev` branch** → Builds `cbaugus/rust-loadtest:dev`
- **Push to `main` branch** → Builds `cbaugus/rust-loadtest:latest`
- **Push tag `v*`** → Builds versioned tags (e.g., `v0.2.0`, `0.2`, `0`)
- **Pull request** → Builds only (doesn't push)
- **Manual trigger** → Via GitHub UI

### Tags Generated

| Event | Tags |
|-------|------|
| `dev` branch push | `dev`, `dev-<sha>` |
| `main` branch push | `latest`, `main-<sha>` |
| Tag `v1.2.3` | `1.2.3`, `1.2`, `1`, `v1.2.3` |

### Multi-Architecture

Builds for:
- `linux/amd64` (x86_64)
- `linux/arm64` (ARM)

### Usage

After the workflow runs, pull the image:

```bash
# Dev branch
docker pull cbaugus/rust-loadtest:dev

# Main/latest
docker pull cbaugus/rust-loadtest:latest

# Specific version
docker pull cbaugus/rust-loadtest:0.2.0
```
