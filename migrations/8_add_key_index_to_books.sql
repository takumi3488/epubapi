create index books_key_index on books using hash ("key");
alter table books add constraint books_key_unique unique ("key");