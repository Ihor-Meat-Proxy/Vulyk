FROM node:25-bookworm AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
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
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html
EXPOSE 8080
CMD ["nginx", "-g", "daemon off;"]
