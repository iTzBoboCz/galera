CREATE TABLE `media` (
	`id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
	`filename` VARCHAR(255) NOT NULL,
	`folder_id` INT NOT NULL,
	`owner_id` INT NOT NULL,
	`width` INT UNSIGNED NOT NULL,
	`height` INT UNSIGNED NOT NULL,
	`date_taken` TIMESTAMP NOT NULL,
	`uuid` VARCHAR(36) NOT NULL UNIQUE,
	`sha2_512` VARCHAR(128) NOT NULL,
	CONSTRAINT `media_fk0` FOREIGN KEY (`folder_id`) REFERENCES `folder`(`id`),
  CONSTRAINT `media_fk1` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`)
);
