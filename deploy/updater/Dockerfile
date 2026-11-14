FROM python:3.12-slim-bookworm
# docker.io from Debian; Compose v2 plugin from GitHub releases (no docker-compose-v2
# apt package on bookworm). Needed so install.sh upgrade/rollback can talk to the
# mounted Docker socket from the updater sidecar.
ARG DOCKER_COMPOSE_VERSION=v5.7.1
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates docker.io gzip \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /usr/local/lib/docker/cli-plugins \
    && curl -fsSL "https://github.com/docker/compose/releases/download/${DOCKER_COMPOSE_VERSION}/docker-compose-linux-x86_64" \
         -o /usr/local/lib/docker/cli-plugins/docker-compose \
    && chmod +x /usr/local/lib/docker/cli-plugins/docker-compose
# Prefer Compose plugin when present
WORKDIR /vulyk
COPY server.py /usr/local/bin/vulyk-updater
RUN chmod +x /usr/local/bin/vulyk-updater
ENV VULYK_UPDATER_BIND=0.0.0.0:9091
EXPOSE 9091
USER root
CMD ["python3", "/usr/local/bin/vulyk-updater"]
