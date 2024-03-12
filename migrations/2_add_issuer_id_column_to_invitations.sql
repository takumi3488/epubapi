-- invitationsテーブルにissuer_idカラムを追加
alter table invitations add column issuer_id text not null references users(id);

-- usersテーブルからinvitation_pointsカラムを削除
alter table users drop column invitation_points;