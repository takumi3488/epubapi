#!/bin/ash

ash -c "/img2epub && /get_metadata && /epub2img"
if [ $? -ne 0 ]; then
    echo "Error: convert failed"
    if [ -n "$FAILURE_NOTIFICATION_URL" ]; then
        curl -X POST -d "$(date -u) - Error: convert failed" $FAILURE_NOTIFICATION_URL
    fi
fi
