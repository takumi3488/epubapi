# epubapi

## できること

- S3互換ストレージの画像をEPUBに変換、EPUBに権限付きAPIを生やす

## 実行ファイル

- `server`: Webサーバー
- `gen_metadata`: 
- `img2epub`: 
- `epub2img`:

## 環境変数

- `DATABASE_URL`: PostgreSQLのURL
- `S3_ENDPOINT`
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `IMAGES_BUCKET`: 画像とメタデータを.tar.gzでまとめたファイルを保管するバケット
- `EPUB_BUCKET`: EPUBファイルを保管するバケット
- `API_KEY`: 連携アプリ用のKey
- `ADMIN_ID`: 起動時に作成される管理者のID
- `ADMIN_PASSWORD`: 起動時に作成される管理者のパスワード
- `JWT_SECRET`

## 操作方法

`task` コマンドで実行（詳細は `Taskfile.yml` を参照）
