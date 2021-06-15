CREATE TABLE `favorite_media` (
	`id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
	`media_id` INT NOT NULL,
	`user_id` INT NOT NULL,
	CONSTRAINT `favorite_media_fk0` FOREIGN KEY (`media_id`) REFERENCES `media`(`id`),
  CONSTRAINT `favorite_media_fk1` FOREIGN KEY (`user_id`) REFERENCES `user`(`id`)
);
