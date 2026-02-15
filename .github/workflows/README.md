# GitHub Actions Workflows

## Overview

This repository has two CI/CD pipelines:

### 1. Main Branch Pipeline (`main-build-cicd.yaml`)
- **Triggers on:** Push to `main` branch, PRs to main
- **Builds:** Two Docker images (standard + Chainguard)
- **Features:** Lint, test, SBOM generation, multi-platform support
- **Tags:** `latest` (main branch)

### 2. Dev Branch Pipeline (`dev-build-cicd.yaml`)
- **Triggers on:** Push to `dev` branch, PRs to dev
- **Builds:** Single Docker image (amd64 only for speed)
- **Features:** Fast builds with caching, artifact attestation
- **Tags:** `dev`, `dev-<sha>`

---

## Setup

### Docker Hub Credentials

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

### Pipeline Details

#### Main Branch (`main-build-cicd.yaml`)
**Triggers:**
- Push to `main` branch
- Pull requests to `main`

**Process:**
1. Lint (rustfmt & clippy)
2. Run test suite
3. Build two Docker images:
   - Standard Ubuntu-based image
   - Minimal Chainguard static image
4. Generate SBOMs for both images
5. Push to Docker Hub

**Images:**
- `cbaugus/rust_loadtest:latest`
- `cbaugus/rust_loadtest:latest-Chainguard`

#### Dev Branch (`dev-build-cicd.yaml`)
**Triggers:**
- Push to `dev` branch
- Pull requests to `dev`
- Manual trigger via GitHub UI

**Process:**
1. Build Docker image (amd64 only)
2. Push to Docker Hub with caching

**Images:**
- `cbaugus/rust_loadtest:dev`
- `cbaugus/rust_loadtest:dev-<git-sha>`

**Platform:**
- `linux/amd64` (x86_64 only - optimized for faster dev builds)

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
