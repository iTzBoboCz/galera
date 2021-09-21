CREATE TABLE `album` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `owner_id` INT NOT NULL,
  `name` varchar(255) NOT NULL,
  `description` varchar(255),
  `created_at` TIMESTAMP NOT NULL,
  `thumbnail_link` VARCHAR(36),
  `link` VARCHAR(21) NOT NULL UNIQUE,
  `password` varchar(255),
  CONSTRAINT `album_fk0` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`),
  CONSTRAINT `album_fk1` FOREIGN KEY (`thumbnail_link`) REFERENCES `media`(`uuid`)
);
