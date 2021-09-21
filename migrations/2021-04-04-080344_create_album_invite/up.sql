CREATE TABLE `album_invite` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `album_id` INT NOT NULL,
  `invited_user_id` INT NOT NULL,
  `accepted` BOOLEAN NOT NULL,
  `write_access` BOOLEAN NOT NULL,
  CONSTRAINT `album_invite_fk0` FOREIGN KEY (`album_id`) REFERENCES `album`(`id`),
  CONSTRAINT `album_invite_fk1` FOREIGN KEY (`invited_user_id`) REFERENCES `user`(`id`)
);
