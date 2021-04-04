CREATE TABLE `favourite_photo` (
	`id` INT NOT NULL PRIMARY KEY,
	`photo_id` INT NOT NULL,
	`user_id` INT NOT NULL,
	CONSTRAINT `favourite_photo_fk0` FOREIGN KEY (`photo_id`) REFERENCES `photo`(`id`),
  CONSTRAINT `favourite_photo_fk1` FOREIGN KEY (`user_id`) REFERENCES `user`(`id`)
);
