FROM node:22-bookworm AS builder
# Build from the repository root so docs/user Markdown can be bundled.
WORKDIR /src
COPY frontend/package.json frontend/package-lock.json frontend/.npmrc ./frontend/
COPY scripts/npm-ci-secure.sh scripts/npm-install-scripts-allowlist.txt ./scripts/
RUN chmod +x scripts/npm-ci-secure.sh && ./scripts/npm-ci-secure.sh frontend
COPY frontend ./frontend
COPY docs/user ./docs/user
WORKDIR /src/frontend
ARG VITE_API_URL=
ENV VITE_API_URL=$VITE_API_URL
RUN npm run build

# Unprivileged nginx (non-root UID 101) — security audit M8.
# Keep current with alpine security patches (Trivy image gate).
FROM nginxinc/nginx-unprivileged:1.31.7-alpine
# Apply Alpine package fixes (e.g. brotli-libs) not yet baked into the tag.
USER root
RUN apk upgrade --no-cache
USER nginx
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /src/frontend/dist /usr/share/nginx/html
EXPOSE 8080
CMD ["nginx", "-g", "daemon off;"]
