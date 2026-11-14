# The TechnoApe dedicated server with its web page (ADR 0130), built on the box that runs it (so the
# repository's target-cpu=native is that box's own CPU). tools/deploy.sh sends the source and runs this.
FROM rust:1.99.1-slim-bookworm AS build
WORKDIR /src
COPY . .
# the caches keep the next deploy's build to what changed
RUN --mount=type=cache,target=/src/target --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked -p server && cp target/release/technoape-server /technoape-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /technoape-server /usr/local/bin/technoape-server
COPY deploy/web.py /web.py
# saves/ (the world, its bake) lands in the volume
WORKDIR /data
VOLUME /data
ENV SEED=1337 PLANET=6144 PORT=7777 WEB_PORT=7780
ENTRYPOINT ["/bin/sh", "-c", "exec python3 /web.py technoape-server listen=0.0.0.0:$PORT seed=$SEED planet=$PLANET \"$@\"", "--"]
