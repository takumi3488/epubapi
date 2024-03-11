#!/bin/sh -e
mc config host add minio http://$MINIO_ENDPOINT $MINIO_ROOT_USER $MINIO_ROOT_PASSWORD
mc ls minio/$IMAGES_BUCKET || mc mb minio/$IMAGES_BUCKET
mc cp /test.tar.gz minio/$IMAGES_BUCKET/test_user_id/test.tar.gz
mc ls minio/$EPUB_BUCKET || mc mb minio/$EPUB_BUCKET
