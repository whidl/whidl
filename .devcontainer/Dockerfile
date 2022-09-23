ARG VARIANT=ubuntu-22.04
FROM mcr.microsoft.com/vscode/devcontainers/base:0-${VARIANT}

# Base
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends build-essential

# Java for nand2tetris
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends openjdk-11-jdk-headless


# Jupyter / data analysis
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends jupyter python3-pip python3-setuptools python3-dev texlive-latex-base dvipng texlive-latex-extra texlive-fonts-recommended cm-super

# From the vs code rust container
# https://github.com/microsoft/vscode-dev-containers/blob/main/containers/rust/.devcontainer/library-scripts/rust-debian.sh
COPY library-scripts/*.sh library-scripts/*.env /tmp/library-scripts/
RUN bash /tmp/library-scripts/rust-debian.sh "${CARGO_HOME}" "${RUSTUP_HOME}" "${USERNAME}" "true" "true" \
    && apt-get clean -y && rm -rf /var/lib/apt/lists/* /tmp/library-scripts

RUN echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] http://packages.cloud.google.com/apt cloud-sdk main" | tee -a /etc/apt/sources.list.d/google-cloud-sdk.list && curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | apt-key --keyring /usr/share/keyrings/cloud.google.gpg  add - && apt-get update -y && apt-get install google-cloud-cli -y

# Quartus Prime
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends libtcmalloc-minimal4
ENV PATH="/opt/intelFPGA_lite/quartus/bin:/opt/intelFPGA_lite/modelsim_ase/bin:${PATH}"

RUN chown vscode /opt

USER vscode
RUN cd /tmp \
   && wget https://downloads.intel.com/akdlm/software/acdsinst/20.1std.1/720/ib_installers/QuartusLiteSetup-20.1.1.720-linux.run \
   && wget https://downloads.intel.com/akdlm/software/acdsinst/20.1std.1/720/ib_installers/ModelSimSetup-20.1.1.720-linux.run \
   && wget https://downloads.intel.com/akdlm/software/acdsinst/20.1std.1/720/ib_installers/cyclonev-20.1.1.720.qdz \
   && chmod +x *.run \
   && ./QuartusLiteSetup-20.1.1.720-linux.run --mode unattended --accept_eula 1 --installdir /opt/intelFPGA_lite \
   && rm *.run

COPY library-scripts/nvm.sh /tmp/library-scripts/
RUN bash /tmp/library-scripts/nvm.sh 

USER root
COPY library-scripts/modelsim.sh /tmp/library-scripts/
RUN bash /tmp/library-scripts/modelsim.sh 

USER vscode
ENV CARGO_HOME /usr/local/cargo
ENV RUSTUP_HOME /usr/local/rustup
RUN /usr/local/cargo/bin/cargo install wasm-pack
