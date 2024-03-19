insert into
    books(
        id,
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
        'user_public_book_key',
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
        'user_private_book_key',
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
        'admin_public_book_key',
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
        'admin_private_book_key',
        'admin_id',
        'admin_private_book_name',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'private'
    ),(
        'test_book_id',
        'minio_user_id/test.epub',
        'minio_user_id',
        'test_epub',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'public'
    );

insert into
    book_tags(book_id, tag_name)
values
    ('user_public_book_id', 'test_tag'),
    ('user_private_book_id', 'test_tag'),
    ('admin_public_book_id', 'test_tag'),
    ('admin_private_book_id', 'test_tag');
