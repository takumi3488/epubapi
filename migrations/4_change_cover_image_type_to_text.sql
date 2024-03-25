-- booksのcover_imageカラムの型をtextに変更
alter table books
    alter column cover_image type text using cover_image::text;
