# Multi-stage Dockerfile for Cortex Documentation Website
# Stage 1: Build the documentation site
FROM node:20-alpine AS builder

WORKDIR /app

# Install dependencies first (for optimal Docker layer caching)
COPY website/package*.json ./website/
WORKDIR /app/website
RUN npm ci || npm install

# Copy documentation markdown (single source of truth) and website source
WORKDIR /app
COPY docs ./docs
COPY website ./website

# Build the static site and Pagefind search indexes
WORKDIR /app/website
RUN npm run build

# Stage 2: Serve the website
FROM node:20-alpine AS runner

WORKDIR /app/website

# Install production dependencies for running the preview server
COPY website/package*.json ./
RUN npm install --omit=dev || npm install

# Copy static distribution build and Astro config
COPY --from=builder /app/website/dist ./dist
COPY --from=builder /app/website/astro.config.mjs ./

# Expose default Starlight port
EXPOSE 4321

ENV HOST=0.0.0.0
ENV PORT=4321

# Run the website
CMD ["npm", "run", "preview", "--", "--host", "0.0.0.0", "--port", "4321"]
