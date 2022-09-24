ARG VARIANT=ubuntu-22.04
FROM mcr.microsoft.com/vscode/devcontainers/base:0-${VARIANT}

# Base
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends build-essential

# From the vs code rust container
# https://github.com/microsoft/vscode-dev-containers/blob/main/containers/rust/.devcontainer/library-scripts/rust-debian.sh
COPY library-scripts/*.sh library-scripts/*.env /tmp/library-scripts/
RUN bash /tmp/library-scripts/rust-debian.sh "${CARGO_HOME}" "${RUSTUP_HOME}" "${USERNAME}" "true" "true" \
    && apt-get clean -y && rm -rf /var/lib/apt/lists/* /tmp/library-scripts

USER vscode
ENV CARGO_HOME /usr/local/cargo
ENV RUSTUP_HOME /usr/local/rustup
RUN /usr/local/cargo/bin/cargo install wasm-pack
