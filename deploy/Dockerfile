# The TechnoApe dedicated server (ADR 0145): the stock Linux Godot binary running the project's server
# script headless. No voxel or sim extension is needed (the server keeps the edit journal and relays);
# the Windows .gdextension libraries are left out of the image.
#   docker build -f deploy/Dockerfile -t technoape-server .        (from the repository root)
FROM debian:bookworm-slim
ARG GODOT_VERSION=4.7.2
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates unzip libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*
ADD https://github.com/godotengine/godot/releases/download/${GODOT_VERSION}-stable/Godot_v${GODOT_VERSION}-stable_linux.x86_64.zip /tmp/godot.zip
RUN unzip -q /tmp/godot.zip -d /opt/godot && mv /opt/godot/Godot_v${GODOT_VERSION}-stable_linux.x86_64 /opt/godot/godot && rm /tmp/godot.zip
COPY game /game
COPY deploy/entry.sh /entry.sh
# the project's import cache, so the first start is not a minute of importing (a missing extension only logs)
RUN /opt/godot/godot --headless --path /game --import > /tmp/import.log 2>&1 || /opt/godot/godot --headless --path /game --import > /tmp/import.log 2>&1 || true
ENV XDG_DATA_HOME=/data
VOLUME /data
EXPOSE 7777/udp 7778/udp
ENTRYPOINT ["/entry.sh"]
