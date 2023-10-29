#!/bin/bash

# This script zips up the current directory, copies it to a remote server, 
# unzips on the remote server, and then builds the project using whidl and quartus prime. 
# The .sof file resulting from the build is then copied back to the local machine 
# so it can be programmed onto your board.

# Set the parameters
ZIP_NAME="archive.zip"
REMOTE_USERNAME="..."
REMOTE_ADDRESS="..."
REMOTE_PATH="quartus_build"
CHIP_NAME=$1

if [ -z $CHIP_NAME ] ;then
	echo "Must provide Chip Name."
	exit
fi

# Create a zip file of the current directory
echo "Zipping up the current directory..."
zip ${ZIP_NAME} *.hdl

# SCP to the remote server -- replace "username@hostname" with your SSH username and hostname
echo "Copying the zip file to the remote server..."
scp ${ZIP_NAME} ${REMOTE_USERNAME}@${REMOTE_ADDRESS}:${ZIP_NAME}

# SSH into the remote server, unzip the file and run make
# Assuming that you'll want to run this on a Linux server
ssh ${REMOTE_USERNAME}@${REMOTE_ADDRESS} <<EOF

  echo "Unzipping the file..."
  rm -fr ${REMOTE_PATH}
  mkdir -p ${REMOTE_PATH}
  mv ${ZIP_NAME} ${REMOTE_PATH}
  cd ${REMOTE_PATH}
  unzip ${ZIP_NAME}
  rm ${ZIP_NAME}
  
  echo "Building the project..."
  whidl synth-vhdl ${CHIP_NAME}.hdl _build
  cd _build
  quartus_sh -t project.tcl
  quartus_sh --flow compile $CHIP_NAME

EOF

scp ${REMOTE_USERNAME}@${REMOTE_ADDRESS}:${REMOTE_PATH}/_build/${CHIP_NAME}.sof .

echo "Done."

