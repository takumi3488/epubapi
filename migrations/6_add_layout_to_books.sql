create type layout as enum ('reflowable', 'pre-paginated');
alter table
    books add column layout layout;
