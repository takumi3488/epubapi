#!/bin/sh -e
mc config host add minio http://$MINIO_ENDPOINT $MINIO_ROOT_USER $MINIO_ROOT_PASSWORD
mc ls minio/$IMAGES_BUCKET || mc mb minio/$IMAGES_BUCKET
for f in /images/test*.tar.gz; do
  mc cp $f minio/$IMAGES_BUCKET/minio_user_id/$(basename $f)
done
mc ls minio/$EPUB_BUCKET || mc mb minio/$EPUB_BUCKET
