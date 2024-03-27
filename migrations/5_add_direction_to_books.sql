-- add direction column to books
create type direction as enum ('ltr', 'rtl');
alter table
    books add column direction direction not null default 'ltr';
