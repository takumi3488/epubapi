insert into
  users (id, password, role)
values
  (
    'admin_id',
    'admin_password',
    'admin'
  );

insert into
  invitations (code, state, used_at, issuer_id)
values
  ('unused_test_code', 'unused', null, 'admin_id'),
  (
    'used_test_code',
    'used',
    '2020-01-01 00:00:00',
    'admin_id'
  );