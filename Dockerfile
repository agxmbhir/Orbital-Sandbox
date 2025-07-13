# --- Frontend build stage ----------------------------------------------------
FROM node:20-alpine AS web-build
WORKDIR /web
COPY web/package*.json ./
RUN npm install
COPY web .
RUN npm run build

# --- Backend build stage ----------------------------------------------------
FROM rust:1.73-alpine AS backend-build
WORKDIR /app
RUN apk add --no-cache musl-dev clang llvm make
COPY --from=web-build /web/dist ./web/dist
COPY orbital ./orbital
WORKDIR /app/orbital
RUN cargo build --release

# --- Runtime stage ----------------------------------------------------------
FROM alpine:3.18
WORKDIR /app
COPY --from=backend-build /app/orbital/target/release/orbital ./orbital
COPY --from=backend-build /app/orbital/web/dist ./web/dist
# The server binds to PORT env or 8080 by default
ENV PORT=8080
EXPOSE 8080
CMD ["/app/orbital", "serve", "--port", "8080"] 