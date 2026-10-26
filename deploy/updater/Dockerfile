FROM python:3.12-slim-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates docker.io docker-compose-v2 gzip \
    && rm -rf /var/lib/apt/lists/*
# Prefer Compose plugin when present
WORKDIR /vulyk
COPY server.py /usr/local/bin/vulyk-updater
RUN chmod +x /usr/local/bin/vulyk-updater
ENV VULYK_UPDATER_BIND=0.0.0.0:9091
EXPOSE 9091
USER root
CMD ["python3", "/usr/local/bin/vulyk-updater"]
