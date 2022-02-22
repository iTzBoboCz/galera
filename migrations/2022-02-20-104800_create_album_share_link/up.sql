CREATE TABLE `album_share_link` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `album_id` INT NOT NULL,
  `uuid` VARCHAR(21) NOT NULL UNIQUE,
  `password` VARCHAR(128),
  `expiration` DATETIME,
  CONSTRAINT `album_share_link_fk0` FOREIGN KEY (`album_id`) REFERENCES `album`(`id`)
);
