-- invitations テーブルの作成
create type invitation_state as enum ('unused', 'using', 'used');

create table invitations (
    code text primary key,
    "state" invitation_state not null default 'unused',
    used_at timestamp default null
);

create index invitations_code_index on invitations using hash (code);

-- users テーブルの作成
create type user_role as enum ('admin', 'user');

create table users (
    id text primary key,
    "password" text not null,
    role user_role not null default 'user',
    api_key text not null default gen_random_uuid(),
    invitation_points integer not null default 0
);

create index users_id_index on users using hash (id);

create index users_api_key_index on users using hash (api_key);

-- books テーブルの作成
create type visibility as enum ('public', 'private');

create table books (
    "key" text primary key,
    owner_id text not null references users(id),
    "name" text not null,
    creator text not null,
    publisher text not null,
    "date" text not null,
    cover_image bytea not null,
    created_at timestamp not null default now(),
    visibility visibility not null default 'private'
);

create index books_key_index on books using hash ("key");

-- tags テーブルの作成
create table tags (
    "name" text primary key,
    created_at timestamp not null default now()
);

-- book_tags テーブルの作成
create table book_tags (
    book_key text not null references books("key") on delete cascade,
    tag_name text not null references tags("name") on delete cascade,
    primary key (book_key, tag_name)
);