ARG VARIANT=ubuntu-22.04
FROM mcr.microsoft.com/vscode/devcontainers/base:0-${VARIANT}

# Base
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends build-essential unzip vim-nox

# Quartus Prime
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends libtcmalloc-minimal4 libglib2.0-0
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

# From the vs code rust container
COPY library-scripts/*.sh /tmp/library-scripts/
RUN sudo bash /tmp/library-scripts/modelsim.sh


