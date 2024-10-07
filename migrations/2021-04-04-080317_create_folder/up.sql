CREATE TABLE `folder` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `uuid` CHAR(36) NOT NULL UNIQUE,
  `owner_id` INT NOT NULL,
  `parent` INT,
  `name` VARCHAR(255) NOT NULL,
  CONSTRAINT `folder_fk0` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`),
  CONSTRAINT `folder_fk1` FOREIGN KEY (`parent`) REFERENCES `folder`(`id`)
);
