CREATE TABLE `album_media` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `album_id` INT NOT NULL,
  `media_id` INT NOT NULL,
  CONSTRAINT `album_media_fk0` FOREIGN KEY (`media_id`) REFERENCES `media`(`id`),
  CONSTRAINT `album_media_fk1` FOREIGN KEY (`album_id`) REFERENCES `album`(`id`),
  CONSTRAINT `album_media_un0` UNIQUE (`album_id`, `media_id`)
);
