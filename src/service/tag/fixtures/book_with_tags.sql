insert into
    books(
        "key",
        owner_id,
        "name",
        creator,
        publisher,
        "date",
        cover_image,
        visibility
    )
values
    (
        'user_public_book_id',
        'user_id',
        'user_public_book_name',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'public'
    ),
    (
        'user_private_book_id',
        'user_id',
        'user_private_book_name',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'private'
    ),
    (
        'admin_public_book_id',
        'admin_id',
        'admin_public_book_name',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'public'
    ),
    (
        'admin_private_book_id',
        'admin_id',
        'admin_private_book_name',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'private'
    );

insert into
    book_tags(book_key, tag_name)
values
    ('user_public_book_id', 'test_tag'),
    ('user_private_book_id', 'test_tag'),
    ('admin_public_book_id', 'test_tag'),
    ('admin_private_book_id', 'test_tag');