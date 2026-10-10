# Cortex Agent Documentation Website

Production-ready documentation website for **Cortex Agent**, built with [Astro](https://astro.build) and [Starlight](https://starlight.astro.build).

## Key Architecture & Design

1. **Canonical Single Source of Truth (`../docs/`)**:
   - The repository's root `docs/` directory remains the sole source of truth for all documentation.
   - No markdown files are moved, renamed, or duplicated.
   - An in-memory Astro Content Layer loader (`cortexDocsLoader` in `src/content.config.ts`) dynamically reads, inspects, and parses every document from `../docs/` at build and dev time.
   - Headings are automatically extracted to supply page titles and descriptions, and relative cross-document links (e.g., `../architecture/tracing.md`) are transformed into clean URL routes (`/architecture/tracing/`).
2. **Codex CLI & Claude Code Visual Theme (`DESIGN.md`)**:
   - Signature terracotta (`#d77757`), shimmer terracotta (`#eb9f7f`), hot pink (`#fd5db1`), and lavender (`#b1b9f9`) palette.
   - Dashed ASCII framing and monospaced typography accents.
   - Responsive dark and light mode support with high-contrast text.
3. **Command-Forward Landing Page (`src/pages/index.astro`)**:
   - Prominently showcases instant CLI installation, environment configuration, agent execution, TUI inspection, and benchmark evaluation commands directly on the home page.
4. **Built-in Search & Offline Indexing**:
   - Full-text search powered by Pagefind indexing all static documentation pages.

---

## Getting Started Locally

### Prerequisites

- [Node.js](https://nodejs.org) >= 18.14.1 (v20+ recommended)
- [npm](https://npmjs.com)

### Installation

```bash
cd website
npm install
```

### Development Server

Start the local development server with hot-module reloading:

```bash
npm run dev
```

Visit `http://localhost:4321` in your browser. Any edits made to files in `../docs/` are automatically detected and reflected in real-time.

---

## Production Build & Preview

### Build Static Site

```bash
npm run build
```

The optimized static assets, HTML pages, CSS bundles, and Pagefind search index are output to `website/dist/`.

### Local Production Preview

To test the production build locally:

```bash
npm run preview
```

---

## Deployment Options

### 1. GitHub Pages

Add a workflow in `.github/workflows/deploy-docs.yml`:

```yaml
name: Deploy Documentation to GitHub Pages

on:
  push:
    branches: [main]
    paths:
      - 'docs/**'
      - 'website/**'
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: 'pages'
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'
          cache-dependency-path: 'website/package-lock.json'
      - name: Install dependencies
        run: cd website && npm ci
      - name: Build Astro website
        run: cd website && npm run build
      - uses: actions/upload-pages-artifact@v3
        with:
          path: website/dist

  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/deploy-pages@v4
        id: deployment
```

### 2. Docker Container

Build and run the documentation container from the repository root:

```bash
docker build -t cortex-docs .
docker run -p 4321:4321 cortex-docs
```

Access the documentation at `http://localhost:4321`.

### 3. Cloudflare Pages / Vercel / Netlify

- **Root Directory**: `website`
- **Build Command**: `npm run build`
- **Output Directory**: `dist`
- **Node.js Version**: 20+
