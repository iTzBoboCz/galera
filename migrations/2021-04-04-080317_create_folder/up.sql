CREATE TABLE `folder` (
	`id` INT NOT NULL PRIMARY KEY,
	`owner_id` INT NOT NULL,
	`parent` INT,
	`name` INT NOT NULL,
	CONSTRAINT `folder_fk0` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`),
  CONSTRAINT `folder_fk1` FOREIGN KEY (`parent`) REFERENCES `folder`(`id`)
);
