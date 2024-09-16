create table adding_tag_tasks (
  id uuid primary key default gen_random_uuid(),
  book_key text not null,
  tags text[] not null,
  created_at timestamptz not null default current_timestamp
);