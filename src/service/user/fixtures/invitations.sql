insert into
    invitations(code, state, used_at, issuer_id)
values
    ('unused_test_code', 'unused', null, 'used_id'),
    (
        'used_test_code',
        'used',
        '2020-01-01 00:00:00',
        'used_id'
    );