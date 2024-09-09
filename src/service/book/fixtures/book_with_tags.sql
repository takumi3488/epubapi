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
        visibility,
        layout,
        images
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
        'public',
        'pre-paginated',
        '{"image1.jpg", "image2.jpg"}'
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
        'private',
        'pre-paginated',
        '{"image1.jpg", "image2.jpg"}'
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
        'public',
        'pre-paginated',
        '{"image1.jpg", "image2.jpg"}'
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
        'private',
        'pre-paginated',
        '{"image1.jpg", "image2.jpg"}'
    ),(
        'test_book_id',
        'minio_user_id/test1.epub',
        'test_user_id',
        'test_epub',
        'book_creator',
        'book_publisher',
        'book_date',
        'book_cover_image',
        'public',
        'pre-paginated',
        '{"image1.jpg", "image2.jpg"}'
    );

insert into
    book_tags(book_id, tag_name)
values
    ('user_public_book_id', 'test_tag'),
    ('user_private_book_id', 'test_tag'),
    ('admin_public_book_id', 'test_tag'),
    ('admin_private_book_id', 'test_tag');
