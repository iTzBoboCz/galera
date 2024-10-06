CREATE TABLE `album_share_link` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `uuid` CHAR(36) NOT NULL UNIQUE,
  `album_id` INT NOT NULL,
  `link` VARCHAR(21) NOT NULL UNIQUE,
  `password` VARCHAR(128),
  `expiration` DATETIME,
  CONSTRAINT `album_share_link_fk0` FOREIGN KEY (`album_id`) REFERENCES `album`(`id`) ON DELETE CASCADE
);
