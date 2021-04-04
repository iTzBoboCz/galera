CREATE TABLE `photo` (
	`id` INT NOT NULL PRIMARY KEY,
	`filename` varchar(255) NOT NULL,
	`folder_id` INT NOT NULL,
	`owner_id` INT NOT NULL,
	`album_id` INT,
	`width` INT NOT NULL,
	`height` INT NOT NULL,
	`date_taken` TIMESTAMP NOT NULL,
	`sha2_512` varchar(128) NOT NULL,
	CONSTRAINT `photo_fk0` FOREIGN KEY (`folder_id`) REFERENCES `folder`(`id`),
  CONSTRAINT `photo_fk1` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`),
  CONSTRAINT `photo_fk2` FOREIGN KEY (`album_id`) REFERENCES `album`(`id`)
);
