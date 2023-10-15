#!/bin/bash

# Bash Script to Locate, Format, and Mount Specific Disks

# Ensure the script is being run as root
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root" 
    exit 1
fi

# Check for sufficient arguments
if [[ "$#" -lt 1 ]]; then
    echo "Usage: $0 [-n] <disk_model>"
    echo "Example: $0 \"Samsung SSD 990 PRO 1TB\""
    exit 1
fi

MOUNT_POINT="/mnt/flashpir"

# Helper function to format and mount disks
perform_operations() {
    index=0
    for disk in $(lsblk -d -o name,model | grep "$2" | awk '{print "/dev/" $1}' | sort); do
        # Format the disk
        if [ "$1" == "true" ]; then
            echo "Dry run: Would format ${disk} and mount it at ${MOUNT_POINT}/${index}"
        else
            echo "Formatting ${disk}..."
            mkfs.ext4 ${disk}

            # Create the mount point and mount the disk
            mount_point="${MOUNT_POINT}/${index}"
            mkdir -p ${mount_point}
            echo "Mounting ${disk} at ${mount_point}..."
            mount ${disk} ${mount_point}
            chmod 777 ${mount_point}
        fi

        # Increment the index
        index=$((index+1))
    done
}

# Check for dry run option
if [[ "$1" == "-n" ]]; then
    if [[ "$#" -ne 2 ]]; then
        echo "Usage: $0 [-n] <disk_model>"
        exit 1
    fi
    echo "Performing a dry run..."
    perform_operations true "$2"
else
    
    perform_operations false "$1"
    echo "Disks formatted."
fi


