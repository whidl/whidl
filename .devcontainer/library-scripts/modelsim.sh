#!/usr/bin/env bash

cd /opt/intelFPGA_lite/modelsim_ase

sed -i 's/linux\_rh[[:digit:]]\+/linux/g' \
    vco
sed -i 's/MTI_VCO_MODE:-""/MTI_VCO_MODE:-"32"/g' \
    vco
sed -i '/dir=`dirname "$arg0"`/a export LD_LIBRARY_PATH=${dir}/lib32' \
    vco

dpkg --add-architecture i386
apt-get update -y
apt-get install -y gcc-multilib g++-multilib lib32z1 \
lib32stdc++6 lib32gcc-s1 libxt6:i386 libxtst6:i386 expat:i386 \
fontconfig:i386 libfreetype6:i386 libexpat1:i386 libc6:i386 \
libgtk-3-0:i386 libcanberra0:i386 libice6:i386 libsm6:i386 \
libncurses5:i386 zlib1g:i386 libx11-6:i386 libxau6:i386 \
libxdmcp6:i386 libxext6:i386 libxft2:i386 libxrender1:i386
