-- keyにつけたindexの削除
drop index books_key_index;

-- keyにつけたprimary keyの削除
alter table
    books drop constraint books_pkey cascade;

-- idカラムの追加
alter table
    books
add
    column id text primary key default gen_random_uuid();

-- book_tagsテーブルのbook_keyカラムの名前を変更
alter table
    book_tags rename column book_key to book_id;